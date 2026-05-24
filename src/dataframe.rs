use std::sync::Arc;

use crate::logical_plan::{
    Aggregate, Join, JoinType, LogicalPlan, Projection, Selection,
    expr::{BinaryOp, Literal, LogicalExpr},
};

#[derive(Debug, Clone)]
pub struct DataFrame {
    plan: LogicalPlan,
}

impl DataFrame {
    pub fn new(plan: LogicalPlan) -> DataFrame {
        DataFrame { plan }
    }

    pub fn plan(&self) -> LogicalPlan {
        self.plan.clone()
    }

    pub fn project(self, expr: Vec<Arc<LogicalExpr>>) -> DataFrame {
        let plan = LogicalPlan::Projection(Projection {
            input: Box::new(self.plan),
            expr,
        });
        DataFrame { plan }
    }

    pub fn filter(self, expr: Arc<LogicalExpr>) -> DataFrame {
        let plan = LogicalPlan::Selection(Selection {
            input: Box::new(self.plan),
            expr,
        });
        DataFrame { plan }
    }

    pub fn aggregate(
        self,
        group_by: Vec<Arc<LogicalExpr>>,
        aggregate: Vec<Arc<LogicalExpr>>,
    ) -> DataFrame {
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
        let plan = match join_type {
            JoinType::Inner | JoinType::Left => LogicalPlan::Join(Join {
                left: Box::new(self.plan),
                right: Box::new(right.plan),
                join_type,
                on,
            }),
            JoinType::Right => {
                let on = on.into_iter().map(|(l, r)| (r, l)).collect();
                LogicalPlan::Join(Join {
                    left: Box::new(right.plan),
                    right: Box::new(self.plan),
                    join_type: JoinType::Left,
                    on,
                })
            }
        };

        DataFrame { plan }
    }
}

pub fn col(name: impl AsRef<str>) -> Arc<LogicalExpr> {
    Arc::new(LogicalExpr::Column(name.as_ref().to_string()))
}

pub fn col_index(index: usize) -> Arc<LogicalExpr> {
    Arc::new(LogicalExpr::ColumnIndex(index))
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

pub fn lit(value: impl Into<Literal>) -> Arc<LogicalExpr> {
    Arc::new(LogicalExpr::Literal(value.into()))
}

impl LogicalExpr {
    pub fn eq(self: Arc<LogicalExpr>, rhs: Arc<LogicalExpr>) -> Arc<LogicalExpr> {
        Arc::new(LogicalExpr::Binary {
            lhs: self,
            op: BinaryOp::Eq,
            rhs,
        })
    }

    pub fn neq(self: Arc<LogicalExpr>, rhs: Arc<LogicalExpr>) -> Arc<LogicalExpr> {
        Arc::new(LogicalExpr::Binary {
            lhs: self,
            op: BinaryOp::Neq,
            rhs,
        })
    }

    pub fn gt(self: Arc<LogicalExpr>, rhs: Arc<LogicalExpr>) -> Arc<LogicalExpr> {
        Arc::new(LogicalExpr::Binary {
            lhs: self,
            op: BinaryOp::Gt,
            rhs,
        })
    }

    pub fn gteq(self: Arc<LogicalExpr>, rhs: Arc<LogicalExpr>) -> Arc<LogicalExpr> {
        Arc::new(LogicalExpr::Binary {
            lhs: self,
            op: BinaryOp::GtEq,
            rhs,
        })
    }

    pub fn lt(self: Arc<LogicalExpr>, rhs: Arc<LogicalExpr>) -> Arc<LogicalExpr> {
        Arc::new(LogicalExpr::Binary {
            lhs: self,
            op: BinaryOp::Lt,
            rhs,
        })
    }

    pub fn lteq(self: Arc<LogicalExpr>, rhs: Arc<LogicalExpr>) -> Arc<LogicalExpr> {
        Arc::new(LogicalExpr::Binary {
            lhs: self,
            op: BinaryOp::LtEq,
            rhs,
        })
    }

    pub fn mult(self: Arc<LogicalExpr>, rhs: Arc<LogicalExpr>) -> Arc<LogicalExpr> {
        Arc::new(LogicalExpr::Binary {
            lhs: self,
            op: BinaryOp::Multiply,
            rhs,
        })
    }

    pub fn alias(self: Arc<LogicalExpr>, alias: impl AsRef<str>) -> Arc<LogicalExpr> {
        Arc::new(LogicalExpr::Alias {
            expr: self,
            alias: alias.as_ref().to_string(),
        })
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use insta::assert_snapshot;

    use crate::{
        dataframe::{col, lit},
        execution::ExecutionContext,
    };

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

        assert_snapshot!(df.plan().to_string(), @"
        Filter: #bonus > 1000
          Projection: #id, #first_name, #last_name, #salary, #salary * 0.1 as bonus
            Filter: #state = 'CO'
              Scan: employee.csv; projection=None
        ");
        Ok(())
    }
}
