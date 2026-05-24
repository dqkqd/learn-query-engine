pub mod data_source;
pub mod dataframe;
pub mod logical_plan;
pub mod optimizer;
pub mod physical_plan;
pub mod query_planner;
pub mod sql;
pub mod utils;

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use anyhow::Result;
    use arrow::array::RecordBatch;

    use crate::{
        data_source::{DataSource, csv::CsvDataSource},
        dataframe::DataFrame,
        logical_plan::{LogicalPlan, Scan},
        physical_plan::PhysicalPlan,
        sql::{expr::SqlExpr, parser::Parser, planner::create_dataframe, tokenizer::Tokenizer},
    };

    pub fn parse(sql: &str) -> Result<SqlExpr> {
        let tokenizer = Tokenizer::new(sql);
        let stream = tokenizer.stream()?;
        let mut parser = Parser::new(stream);
        let expr = parser.parse()?;
        Ok(expr)
    }

    pub fn plan(sql: &str) -> Result<LogicalPlan> {
        let plan = LogicalPlan::Scan(Scan {
            path: "employee".to_string(),
            data_source: DataSource::Csv(CsvDataSource::new("test_data/employee.csv")),
            projection: vec![],
        });
        let sql_expr = parse(sql)?;
        let tables = DataFrame::new(plan);
        let tables = HashMap::from([("employee".to_string(), tables)]);

        let df = create_dataframe(sql_expr, tables)?;
        Ok(df.plan())
    }

    pub fn execute_physical_plan(plan: PhysicalPlan) -> Result<Vec<RecordBatch>> {
        plan.execute()?.map(|b| b.map_err(Into::into)).collect()
    }
}
