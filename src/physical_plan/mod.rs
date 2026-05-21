use std::{collections::BTreeMap, sync::Arc};

use anyhow::Result;
use arrow::{
    array::{ArrayRef, AsArray, RecordBatch, new_null_array},
    compute::filter_record_batch,
    row::{OwnedRow, RowConverter, SortField},
};
use arrow_schema::{ArrowError, Schema};

use crate::{
    data_source::DataSource,
    physical_plan::{
        aggregate::{Accumulator, PhysicalAggregateExpr},
        expr::PhysicalExpr,
    },
    utils::field_ids_by_names,
};

pub mod aggregate;
pub mod expr;

pub enum PhysicalPlan {
    Scan(ScanExec),
    Projection(ProjectionExec),
    Selection(SelectionExec),
    HashAggregate(HashAggregrateExec),
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

pub struct HashAggregrateExec {
    schema: Arc<Schema>,
    input: Box<PhysicalPlan>,
    group_expr: Vec<PhysicalExpr>,
    aggregate_expr: Vec<PhysicalAggregateExpr>,
}

impl PhysicalPlan {
    pub fn schema(&self) -> Result<Arc<Schema>> {
        match self {
            PhysicalPlan::Scan(scan_exec) => scan_exec.schema(),
            PhysicalPlan::Projection(projection_exec) => projection_exec.schema(),
            PhysicalPlan::Selection(selection_exec) => selection_exec.schema(),
            PhysicalPlan::HashAggregate(hash_aggregrate_exec) => hash_aggregrate_exec.schema(),
        }
    }

    pub fn children(&self) -> Vec<&PhysicalPlan> {
        match self {
            PhysicalPlan::Scan(scan_exec) => scan_exec.children(),
            PhysicalPlan::Projection(projection_exec) => projection_exec.children(),
            PhysicalPlan::Selection(selection_exec) => selection_exec.children(),
            PhysicalPlan::HashAggregate(hash_aggregrate_exec) => hash_aggregrate_exec.children(),
        }
    }

    pub fn execute(
        &self,
    ) -> Result<Box<dyn Iterator<Item = Result<RecordBatch, ArrowError>> + '_>> {
        match self {
            PhysicalPlan::Scan(scan_exec) => scan_exec.execute(),
            PhysicalPlan::Projection(projection_exec) => projection_exec.execute(),
            PhysicalPlan::Selection(selection_exec) => selection_exec.execute(),
            PhysicalPlan::HashAggregate(hash_aggregrate_exec) => hash_aggregrate_exec.execute(),
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

impl HashAggregrateExec {
    pub fn schema(&self) -> Result<Arc<Schema>> {
        Ok(Arc::clone(&self.schema))
    }

    fn children(&self) -> Vec<&PhysicalPlan> {
        vec![&self.input]
    }

    fn execute(&self) -> Result<Box<dyn Iterator<Item = Result<RecordBatch, ArrowError>> + '_>> {
        let mut map: BTreeMap<OwnedRow, Vec<Accumulator>> = BTreeMap::new();
        let data_types = self
            .schema
            .fields()
            .iter()
            .map(|f| f.data_type())
            .collect::<Vec<_>>();

        let group_by_data_types = data_types
            .iter()
            .map(|&d| SortField::new(d.clone()))
            .take(self.group_expr.len())
            .collect::<Vec<_>>();
        let group_by_row_converter = RowConverter::new(group_by_data_types)?;

        for batch in self.input.execute()? {
            let batch = batch?;

            let aggregate_input: Result<Vec<ArrayRef>, ArrowError> = self
                .aggregate_expr
                .iter()
                .map(|v| v.input().evaluate(&batch))
                .collect();
            let aggregate_input = aggregate_input?;

            let group_keys: Result<Vec<ArrayRef>, ArrowError> = self
                .group_expr
                .iter()
                .map(|expr| expr.evaluate(&batch))
                .collect();
            let group_keys = group_keys?;
            let rows = group_by_row_converter.convert_columns(&group_keys)?;

            for (row_index, row) in rows.iter().enumerate() {
                let row_key = row.owned();
                let accumulators = map.entry(row_key).or_insert_with(|| {
                    self.aggregate_expr
                        .iter()
                        .map(|e| e.accumulator())
                        .collect()
                });

                for (input, accum) in aggregate_input.iter().zip(accumulators.iter_mut()) {
                    let row = input.slice(row_index, 1);
                    accum.accumulate(row)?;
                }
            }
        }

        // group by columns
        let keys = map.keys().map(|r| r.row()).collect::<Vec<_>>();
        let mut columns = group_by_row_converter.convert_rows(keys)?;

        let aggregated_rows = map
            .values()
            .map(|accumulators| {
                accumulators
                    .iter()
                    .map(|accum| accum.value())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        for column_index in 0..self.aggregate_expr.len() {
            let data_type = data_types[self.group_expr.len() + column_index];
            let column = aggregated_rows
                .iter()
                .map(|row| match &row[column_index] {
                    Some(v) => v.clone(),
                    None => new_null_array(data_type, 1),
                })
                .collect::<Vec<_>>();
            let column = column.iter().map(|v| v.as_ref()).collect::<Vec<_>>();
            let column = arrow::compute::kernels::concat::concat(&column)?;
            columns.push(column)
        }

        let batch = RecordBatch::try_new(Arc::clone(&self.schema), columns)?;
        Ok(Box::new(std::iter::once(Ok(batch))))
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

    #[test]
    fn aggregate_exec() -> Result<()> {
        // SELECT MAX(students) GROUP BY class;
        let data_source = data_source(
            r#"
class,group,students
A,one,10
A,two,20
A,three,40
B,one,30
B,two,30
B,three,20
"#,
        )?;
        let scan = PhysicalPlan::Scan(ScanExec {
            data_source: Arc::new(data_source),
            projection: vec![],
        });

        // the returned schema, contains class and max(students)
        let schema = Schema::new(vec![
            Field::new("class", DataType::Utf8, false),
            Field::new("max(students)", DataType::Int64, false),
        ]);

        // column: class
        let group_expr = vec![PhysicalExpr::Column(0)];
        // max(students)
        let aggregate_expr = vec![PhysicalAggregateExpr::Max(PhysicalExpr::Column(2))];

        let aggregation = PhysicalPlan::HashAggregate(HashAggregrateExec {
            schema: Arc::new(schema),
            input: Box::new(scan),
            group_expr,
            aggregate_expr,
        });
        let batch = aggregation
            .execute()?
            .map(|v| v.unwrap())
            .collect::<Vec<_>>();
        assert_snapshot!(pretty_format_batches(&batch)?, @"
        +-------+---------------+
        | class | max(students) |
        +-------+---------------+
        | A     | 40            |
        | B     | 30            |
        +-------+---------------+
        ");
        Ok(())
    }

    #[test]
    fn aggregate_exec_multiple_group_bys_multiple_aggragate() -> Result<()> {
        // SELECT MAX(avg_points), SUM(students) GROUP BY class, group;
        let data_source = data_source(
            r#"
class,group,students,avg_points
A,one,10,1
A,one,20,2
A,two,40,4
B,one,30,3
B,one,30,3
B,two,20,2
"#,
        )?;
        let scan = PhysicalPlan::Scan(ScanExec {
            data_source: Arc::new(data_source),
            projection: vec![],
        });

        // the returned schema, contains class and max(students)
        let schema = Schema::new(vec![
            Field::new("class", DataType::Utf8, false),
            Field::new("group", DataType::Utf8, false),
            Field::new("max(avg_points)", DataType::Int64, false),
            Field::new("sum(students)", DataType::Int64, false),
        ]);

        // column: class
        let group_expr = vec![PhysicalExpr::Column(0), PhysicalExpr::Column(1)];
        // max(avg_points), sum(students)
        let aggregate_expr = vec![
            PhysicalAggregateExpr::Max(PhysicalExpr::Column(3)),
            PhysicalAggregateExpr::Sum(PhysicalExpr::Column(2)),
        ];

        let aggregation = PhysicalPlan::HashAggregate(HashAggregrateExec {
            schema: Arc::new(schema),
            input: Box::new(scan),
            group_expr,
            aggregate_expr,
        });
        let batch = aggregation
            .execute()?
            .map(|v| v.unwrap())
            .collect::<Vec<_>>();
        assert_snapshot!(pretty_format_batches(&batch)?, @"
        +-------+-------+-----------------+---------------+
        | class | group | max(avg_points) | sum(students) |
        +-------+-------+-----------------+---------------+
        | A     | one   | 2               | 30            |
        | A     | two   | 4               | 40            |
        | B     | one   | 3               | 60            |
        | B     | two   | 2               | 20            |
        +-------+-------+-----------------+---------------+
        ");
        Ok(())
    }
}
