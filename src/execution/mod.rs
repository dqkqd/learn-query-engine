use std::collections::HashMap;

use anyhow::Result;
use arrow::array::RecordBatch;
use arrow_schema::ArrowError;

use crate::{
    data_source::{DataSource, csv::CsvDataSource, parquet::ParquetDataSource},
    dataframe::DataFrame,
    logical_plan::{LogicalPlan, Scan},
    optimizer::optimize,
    query_planner::create_physical_plan,
    sql::{parser::Parser, planner::create_dataframe, tokenizer::Tokenizer},
};

#[derive(Default)]
pub struct ExecutionContext {
    tables: HashMap<String, DataFrame>,
}

impl ExecutionContext {
    pub fn sql(&self, sql: impl AsRef<str>) -> Result<DataFrame> {
        let tokenizer = Tokenizer::new(sql);
        let stream = tokenizer.stream()?;
        let mut parser = Parser::new(stream);
        let expr = parser.parse()?;
        create_dataframe(expr, &self.tables)
    }

    pub fn register(&mut self, table_name: impl AsRef<str>, df: DataFrame) {
        self.tables.insert(table_name.as_ref().to_string(), df);
    }

    pub fn csv(filename: impl AsRef<str>) -> Result<DataFrame> {
        let data_source = DataSource::Csv(CsvDataSource::new(&filename));
        let plan = LogicalPlan::Scan(Scan {
            path: filename.as_ref().to_string(),
            data_source,
            projection: vec![],
        });
        Ok(DataFrame::new(plan))
    }

    pub fn register_csv(
        &mut self,
        table_name: impl AsRef<str>,
        file_name: impl AsRef<str>,
    ) -> Result<()> {
        let df = ExecutionContext::csv(file_name)?;
        self.register(table_name, df);
        Ok(())
    }

    pub fn parquet(filename: impl AsRef<str>) -> Result<DataFrame> {
        let data_source = DataSource::Parquet(ParquetDataSource::new(&filename));
        let plan = LogicalPlan::Scan(Scan {
            path: filename.as_ref().to_string(),
            data_source,
            projection: vec![],
        });
        Ok(DataFrame::new(plan))
    }

    pub fn execute(
        self,
        plan: LogicalPlan,
    ) -> Result<Box<dyn Iterator<Item = Result<RecordBatch, ArrowError>>>> {
        let plan = optimize(plan)?;
        let plan = create_physical_plan(&plan)?;
        plan.execute()
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use arrow::util::pretty::pretty_format_batches;
    use arrow_schema::ArrowError;
    use insta::assert_snapshot;

    use crate::execution::ExecutionContext;

    #[test]
    fn employee() -> Result<()> {
        let mut ctx = ExecutionContext::default();
        ctx.register_csv("employee", "test_data/employee.csv")?;
        let sql = "SELECT state, SUM(salary) FROM employee GROUP BY state";
        let df = ctx.sql(sql)?;
        let batches = ctx
            .execute(df.plan())?
            .collect::<Result<Vec<_>, ArrowError>>()?;
        assert_snapshot!( pretty_format_batches(&batches)?, @"
        +-------+-------+
        | state | SUM   |
        +-------+-------+
        |       | 11500 |
        | CA    | 12000 |
        | CO    | 21500 |
        +-------+-------+
        ");
        Ok(())
    }
}
