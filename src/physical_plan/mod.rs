use std::sync::Arc;

use anyhow::Result;
use arrow::{
    array::{ArrayRef, AsArray, RecordBatch},
    compute::filter_record_batch,
};
use arrow_schema::{ArrowError, Schema};

use crate::{
    data_source::DataSource, physical_plan::expr::PhysicalExpr, utils::field_ids_by_names,
};

pub mod expr;

pub enum PhysicalPlan {
    Scan(ScanExec),
    Projection(ProjectionExec),
    Selection(SelectionExec),
}

pub struct ScanExec {
    pub data_source: Arc<dyn DataSource>,
    pub projection: Vec<String>,
}

pub struct ProjectionExec {
    schema: Arc<Schema>,
    input: Box<PhysicalPlan>,
    expr: Vec<PhysicalExpr>,
}

pub struct SelectionExec {
    input: Box<PhysicalPlan>,
    expr: PhysicalExpr,
}

impl PhysicalPlan {
    pub fn schema(&self) -> Result<Arc<Schema>> {
        match self {
            PhysicalPlan::Scan(scan_exec) => scan_exec.schema(),
            PhysicalPlan::Projection(projection_exec) => projection_exec.schema(),
            PhysicalPlan::Selection(selection_exec) => selection_exec.schema(),
        }
    }

    pub fn children(&self) -> Vec<&PhysicalPlan> {
        match self {
            PhysicalPlan::Scan(scan_exec) => scan_exec.children(),
            PhysicalPlan::Projection(projection_exec) => projection_exec.children(),
            PhysicalPlan::Selection(selection_exec) => selection_exec.children(),
        }
    }

    pub fn execute(
        &self,
    ) -> Result<Box<dyn Iterator<Item = Result<RecordBatch, ArrowError>> + '_>> {
        match self {
            PhysicalPlan::Scan(scan_exec) => scan_exec.execute(),
            PhysicalPlan::Projection(projection_exec) => projection_exec.execute(),
            PhysicalPlan::Selection(selection_exec) => selection_exec.execute(),
        }
    }
}

impl ScanExec {
    pub fn schema(&self) -> Result<Arc<Schema>> {
        let schema = self.data_source.schema()?;
        let field_ids = field_ids_by_names(&schema, &self.projection)?;
        let schema = schema.project(&field_ids)?;
        Ok(Arc::new(schema))
    }

    fn children(&self) -> Vec<&PhysicalPlan> {
        vec![]
    }

    fn execute(&self) -> Result<Box<dyn Iterator<Item = Result<RecordBatch, ArrowError>> + '_>> {
        // TODO: clone?
        self.data_source.scan(self.projection.clone())
    }
}

impl ProjectionExec {
    pub fn schema(&self) -> Result<Arc<Schema>> {
        Ok(Arc::clone(&self.schema))
    }

    fn children(&self) -> Vec<&PhysicalPlan> {
        vec![&self.input]
    }

    fn execute(&self) -> Result<Box<dyn Iterator<Item = Result<RecordBatch, ArrowError>> + '_>> {
        let result = self.input.execute()?;
        let result = result.map(|batch| {
            let batch = batch?;
            let columns: Result<Vec<ArrayRef>, ArrowError> =
                self.expr.iter().map(|e| e.evaluate(&batch)).collect();
            let columns = columns?;
            RecordBatch::try_new(Arc::clone(&self.schema), columns)
        });
        Ok(Box::new(result))
    }
}

impl SelectionExec {
    pub fn schema(&self) -> Result<Arc<Schema>> {
        self.input.schema()
    }

    fn children(&self) -> Vec<&PhysicalPlan> {
        vec![&self.input]
    }

    fn execute(&self) -> Result<Box<dyn Iterator<Item = Result<RecordBatch, ArrowError>> + '_>> {
        let result = self.input.execute()?;
        let result = result.map(|batch| {
            let batch = batch?;
            let predicate = self.expr.evaluate(&batch)?;
            let predicate = predicate.as_boolean();
            filter_record_batch(&batch, predicate)
        });
        Ok(Box::new(result))
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use arrow::util::pretty::pretty_format_batches;
    use arrow_schema::{DataType, Field};
    use insta::assert_snapshot;
    use std::io::Write;
    use tempfile::NamedTempFile;

    use crate::{
        data_source::{csv::CsvDataSource, memory::MemoryDataSource},
        physical_plan::expr::{PhysicalBinaryExpr, PhysicalLiteralExpr},
    };

    use super::*;

    fn data_source(data: &str) -> Result<MemoryDataSource> {
        let mut file = NamedTempFile::new()?;
        file.write_all(data.trim().as_bytes())?;
        let path = file.path();
        let filename = path.to_str().unwrap();
        let csv = CsvDataSource::new(filename);
        let mut records = vec![];
        for r in csv.scan(vec![])? {
            let r = r?;
            records.push(r);
        }
        let schema = records[0].schema();
        let memory = MemoryDataSource::new(schema, records);
        Ok(memory)
    }

    #[test]
    fn scan_csv() -> Result<()> {
        let data_source = data_source(
            r#"
column1,column2
1,one
2,two
3,three
4,four
5,five
6,six
"#,
        )?;
        let plan = PhysicalPlan::Scan(ScanExec {
            data_source: Arc::new(data_source),
            projection: vec![],
        });
        let batch = plan.execute()?.map(|v| v.unwrap()).collect::<Vec<_>>();
        assert_snapshot!(pretty_format_batches(&batch)?, @"
        +---------+---------+
        | column1 | column2 |
        +---------+---------+
        | 1       | one     |
        | 2       | two     |
        | 3       | three   |
        | 4       | four    |
        | 5       | five    |
        | 6       | six     |
        +---------+---------+
        ");
        Ok(())
    }

    #[test]
    fn scan_csv_projection() -> Result<()> {
        let data_source = data_source(
            r#"
column1,column2
1,one
2,two
3,three
4,four
5,five
6,six
"#,
        )?;
        let scan = PhysicalPlan::Scan(ScanExec {
            data_source: Arc::new(data_source),
            projection: vec!["column1".to_string()],
        });
        let batch = scan.execute()?.map(|v| v.unwrap()).collect::<Vec<_>>();
        assert_snapshot!(pretty_format_batches(&batch)?, @"
        +---------+
        | column1 |
        +---------+
        | 1       |
        | 2       |
        | 3       |
        | 4       |
        | 5       |
        | 6       |
        +---------+
        ");
        Ok(())
    }

    #[test]
    fn column_expr() -> Result<()> {
        let data_source = data_source(
            r#"
column1,column2
1,one
2,two
3,three
4,four
5,five
6,six
"#,
        )?;
        let scan = PhysicalPlan::Scan(ScanExec {
            data_source: Arc::new(data_source),
            projection: vec![],
        });
        let schema = Schema::new(vec![Field::new("from_column_2", DataType::Utf8, false)]);
        let expr = PhysicalExpr::Column(1);
        let projection = PhysicalPlan::Projection(ProjectionExec {
            schema: Arc::new(schema),
            input: Box::new(scan),
            expr: vec![expr],
        });
        let batch = projection
            .execute()?
            .map(|v| v.unwrap())
            .collect::<Vec<_>>();
        assert_snapshot!(pretty_format_batches(&batch)?, @"
        +---------------+
        | from_column_2 |
        +---------------+
        | one           |
        | two           |
        | three         |
        | four          |
        | five          |
        | six           |
        +---------------+
        ");
        Ok(())
    }

    #[test]
    fn literal() -> Result<()> {
        let data_source = data_source(
            r#"
column1,column2
1,one
2,two
3,three
4,four
5,five
6,six
"#,
        )?;
        let scan = PhysicalPlan::Scan(ScanExec {
            data_source: Arc::new(data_source),
            projection: vec![],
        });
        let schema = Schema::new(vec![
            Field::new("lit_string", DataType::Utf8, false),
            Field::new("lit_long", DataType::Int64, false),
            Field::new("lit_float", DataType::Float64, false),
        ]);
        let expr = vec![
            PhysicalExpr::Literal(PhysicalLiteralExpr::String("lit string".to_string())),
            PhysicalExpr::Literal(PhysicalLiteralExpr::Long(10)),
            PhysicalExpr::Literal(PhysicalLiteralExpr::Double(5.0)),
        ];
        let projection = PhysicalPlan::Projection(ProjectionExec {
            schema: Arc::new(schema),
            input: Box::new(scan),
            expr,
        });
        let batch = projection
            .execute()?
            .map(|v| v.unwrap())
            .collect::<Vec<_>>();
        assert_snapshot!(pretty_format_batches(&batch)?, @"
        +------------+----------+-----------+
        | lit_string | lit_long | lit_float |
        +------------+----------+-----------+
        | lit string | 10       | 5.0       |
        | lit string | 10       | 5.0       |
        | lit string | 10       | 5.0       |
        | lit string | 10       | 5.0       |
        | lit string | 10       | 5.0       |
        | lit string | 10       | 5.0       |
        +------------+----------+-----------+
        ");
        Ok(())
    }

    #[test]
    fn binary_eq_expr() -> Result<()> {
        let data_source = data_source(
            r#"
column1,column2
1,one
1,one
2,two
1,one
2,two
3,three
"#,
        )?;
        let scan = PhysicalPlan::Scan(ScanExec {
            data_source: Arc::new(data_source),
            projection: vec![],
        });

        let schema = Schema::new(vec![Field::new("eq", DataType::Boolean, false)]);
        let expr = PhysicalExpr::Binary(PhysicalBinaryExpr {
            lhs: Arc::new(PhysicalExpr::Column(1)),
            op: expr::PhysicalBinaryOp::Eq,
            rhs: Arc::new(PhysicalExpr::Literal(expr::PhysicalLiteralExpr::String(
                "one".to_string(),
            ))),
        });
        let projection = PhysicalPlan::Projection(ProjectionExec {
            schema: Arc::new(schema),
            input: Box::new(scan),
            expr: vec![expr],
        });
        let batch = projection
            .execute()?
            .map(|v| v.unwrap())
            .collect::<Vec<_>>();
        assert_snapshot!(pretty_format_batches(&batch)?, @"
        +-------+
        | eq    |
        +-------+
        | true  |
        | true  |
        | false |
        | true  |
        | false |
        | false |
        +-------+
        ");
        Ok(())
    }

    #[test]
    fn selection_exec() -> Result<()> {
        let data_source = data_source(
            r#"
column1,column2
a,one
b,one
c,two
d,one
e,two
f,three
"#,
        )?;
        let scan = PhysicalPlan::Scan(ScanExec {
            data_source: Arc::new(data_source),
            projection: vec![],
        });

        let expr = PhysicalExpr::Binary(PhysicalBinaryExpr {
            lhs: Arc::new(PhysicalExpr::Column(1)),
            op: expr::PhysicalBinaryOp::Eq,
            rhs: Arc::new(PhysicalExpr::Literal(expr::PhysicalLiteralExpr::String(
                "one".to_string(),
            ))),
        });
        let selection = PhysicalPlan::Selection(SelectionExec {
            input: Box::new(scan),
            expr,
        });
        let batch = selection.execute()?.map(|v| v.unwrap()).collect::<Vec<_>>();
        assert_snapshot!(pretty_format_batches(&batch)?, @"
        +---------+---------+
        | column1 | column2 |
        +---------+---------+
        | a       | one     |
        | b       | one     |
        | d       | one     |
        +---------+---------+
        ");
        Ok(())
    }
}
