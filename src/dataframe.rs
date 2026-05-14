use anyhow::Result;

use crate::{
    data_source::{csv::CsvDataSource, parquet::ParquetDataSource},
    logical_plan::{
        Aggregate, Join, JoinType, LogicalPlan, Projection, Scan, Selection,
        expr::{BinaryOp, Literal, LogicalExpr},
    },
};

pub struct DataFrame {
    plan: LogicalPlan,
}

pub struct ExecutionContext {}

impl ExecutionContext {
    pub fn csv(filename: impl AsRef<str>) -> Result<DataFrame> {
        let data_source = CsvDataSource::new(&filename);
        let plan = LogicalPlan::Scan(Scan {
            path: filename.as_ref().to_string(),
            data_source: Box::new(data_source),
            projection: vec![],
        });
        Ok(DataFrame { plan })
    }

    pub fn parquet(filename: impl AsRef<str>) -> Result<DataFrame> {
        let data_source = ParquetDataSource::new(&filename);
        let plan = LogicalPlan::Scan(Scan {
            path: filename.as_ref().to_string(),
            data_source: Box::new(data_source),
            projection: vec![],
        });
        Ok(DataFrame { plan })
    }
}

impl DataFrame {
    pub fn project(self, expr: Vec<LogicalExpr>) -> DataFrame {
        let plan = LogicalPlan::Projection(Projection {
            input: Box::new(self.plan),
            expr,
        });
        DataFrame { plan }
    }

    pub fn filter(self, expr: LogicalExpr) -> DataFrame {
        let plan = LogicalPlan::Selection(Selection {
            input: Box::new(self.plan),
            expr,
        });
        DataFrame { plan }
    }

    pub fn aggregate(self, group_by: Vec<LogicalExpr>, aggregate: Vec<LogicalExpr>) -> DataFrame {
        let plan = LogicalPlan::Aggregate(Aggregate {
            input: Box::new(self.plan),
            group_expr: group_by,
            aggregate_expr: aggregate,
        });
        DataFrame { plan }
    }

    pub fn join(
        self,
        right: DataFrame,
        join_type: JoinType,
        on: Vec<(String, String)>,
    ) -> DataFrame {
        let plan = LogicalPlan::Join(Join {
            left: Box::new(self.plan),
            right: Box::new(right.plan),
            join_type,
            on,
        });
        DataFrame { plan }
    }
}

pub fn col(name: impl AsRef<str>) -> LogicalExpr {
    LogicalExpr::Column(name.as_ref().to_string())
}

impl From<&str> for Literal {
    fn from(value: &str) -> Self {
        Literal::String(value.to_string())
    }
}

impl From<String> for Literal {
    fn from(value: String) -> Self {
        Literal::String(value)
    }
}

impl From<i64> for Literal {
    fn from(value: i64) -> Self {
        Literal::Long(value)
    }
}

impl From<f64> for Literal {
    fn from(value: f64) -> Self {
        Literal::Double(value)
    }
}

pub fn lit(value: impl Into<Literal>) -> LogicalExpr {
    LogicalExpr::Literal(value.into())
}

impl LogicalExpr {
    pub fn eq(self, rhs: LogicalExpr) -> LogicalExpr {
        LogicalExpr::Binary {
            left: Box::new(self),
            op: BinaryOp::Eq,
            right: Box::new(rhs),
        }
    }

    pub fn neq(self, rhs: LogicalExpr) -> LogicalExpr {
        LogicalExpr::Binary {
            left: Box::new(self),
            op: BinaryOp::Neq,
            right: Box::new(rhs),
        }
    }

    pub fn gt(self, rhs: LogicalExpr) -> LogicalExpr {
        LogicalExpr::Binary {
            left: Box::new(self),
            op: BinaryOp::Gt,
            right: Box::new(rhs),
        }
    }

    pub fn gteq(self, rhs: LogicalExpr) -> LogicalExpr {
        LogicalExpr::Binary {
            left: Box::new(self),
            op: BinaryOp::GtEq,
            right: Box::new(rhs),
        }
    }

    pub fn lt(self, rhs: LogicalExpr) -> LogicalExpr {
        LogicalExpr::Binary {
            left: Box::new(self),
            op: BinaryOp::Lt,
            right: Box::new(rhs),
        }
    }
    pub fn lteq(self, rhs: LogicalExpr) -> LogicalExpr {
        LogicalExpr::Binary {
            left: Box::new(self),
            op: BinaryOp::LtEq,
            right: Box::new(rhs),
        }
    }

    pub fn mult(self, rhs: LogicalExpr) -> LogicalExpr {
        LogicalExpr::Binary {
            left: Box::new(self),
            op: BinaryOp::Mult,
            right: Box::new(rhs),
        }
    }

    pub fn alias(self, alias: impl AsRef<str>) -> LogicalExpr {
        LogicalExpr::Alias {
            expr: Box::new(self),
            alias: alias.as_ref().to_string(),
        }
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use insta::assert_snapshot;

    use crate::dataframe::{ExecutionContext, col, lit};

    #[test]
    fn test_dataframe() -> Result<()> {
        let ctx = ExecutionContext::csv("employee.csv")?;
        let df = ctx
            .filter(col("state").eq(lit("CO")))
            .project(vec![
                col("id"),
                col("first_name"),
                col("last_name"),
                col("salary"),
                col("salary").mult(lit(0.1)).alias("bonus"),
            ])
            .filter(col("bonus").gt(lit(1000)));

        assert_snapshot!(df.plan.to_string(), @"
        Filter: #bonus > 1000
          Projection: #id, #first_name, #last_name, #salary, #salary * 0.1 as bonus
            Filter: #state = 'CO'
              Scan: employee.csv; projection=None
        ");
        Ok(())
    }
}
