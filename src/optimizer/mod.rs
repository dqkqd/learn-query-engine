use std::collections::BTreeSet;

use anyhow::Result;

use crate::logical_plan::{
    Aggregate, Join, LogicalPlan, Projection, Scan, Selection, expr::LogicalExpr,
};

pub fn optimize(plan: LogicalPlan) -> Result<LogicalPlan> {
    let rules = vec![OptimizerRule::ProjectionPushDown];
    let mut plan = plan;
    for rule in rules {
        plan = rule.optimize(&plan)?;
    }
    Ok(plan)
}

pub enum OptimizerRule {
    ProjectionPushDown,
}

impl OptimizerRule {
    pub fn optimize(&self, plan: &LogicalPlan) -> Result<LogicalPlan> {
        match self {
            OptimizerRule::ProjectionPushDown => projection_push_down(plan),
        }
    }
}

fn projection_push_down(plan: &LogicalPlan) -> Result<LogicalPlan> {
    let mut accum = BTreeSet::new();
    push_down(plan, &mut accum)
}

fn push_down(plan: &LogicalPlan, column_names: &mut BTreeSet<String>) -> Result<LogicalPlan> {
    let plan = match &plan {
        LogicalPlan::Scan(scan) => {
            let projection = scan
                .data_source
                .schema()?
                .fields()
                .iter()
                .map(|f| f.name().clone())
                .filter(|name| column_names.contains(name))
                .collect::<Vec<_>>();
            LogicalPlan::Scan(Scan {
                path: scan.path.clone(),
                data_source: scan.data_source.clone(),
                projection,
            })
        }
        LogicalPlan::Selection(selection) => {
            extract_columns(&selection.expr, plan, column_names)?;
            let input = push_down(&selection.input, column_names)?;
            LogicalPlan::Selection(Selection {
                input: Box::new(input),
                expr: selection.expr.clone(),
            })
        }
        LogicalPlan::Projection(projection) => {
            for expr in &projection.expr {
                extract_columns(expr, &projection.input, column_names)?;
            }
            let input = push_down(&projection.input, column_names)?;
            LogicalPlan::Projection(Projection {
                input: Box::new(input),
                expr: projection.expr.clone(),
            })
        }
        LogicalPlan::Aggregate(aggregate) => {
            for expr in &aggregate.group_expr {
                extract_columns(expr, &aggregate.input, column_names)?;
            }
            for expr in &aggregate.aggregate_expr {
                extract_columns(expr, &aggregate.input, column_names)?;
            }
            let input = push_down(&aggregate.input, column_names)?;
            LogicalPlan::Aggregate(Aggregate {
                input: Box::new(input),
                group_expr: aggregate.group_expr.clone(),
                aggregate_expr: aggregate.aggregate_expr.clone(),
            })
        }
        LogicalPlan::Join(join) => {
            if column_names.is_empty() {
                join.left.schema()?.fields.iter().for_each(|f| {
                    column_names.insert(f.name().clone());
                });
                join.right.schema()?.fields.iter().for_each(|f| {
                    column_names.insert(f.name().clone());
                });
            }
            join.on.iter().for_each(|(l, r)| {
                column_names.insert(l.clone());
                column_names.insert(r.clone());
            });
            let left = push_down(&join.left, column_names)?;
            let right = push_down(&join.right, column_names)?;
            LogicalPlan::Join(Join {
                left: Box::new(left),
                right: Box::new(right),
                join_type: join.join_type.clone(),
                on: join.on.clone(),
            })
        }
    };
    Ok(plan)
}

fn extract_columns(
    expr: &LogicalExpr,
    input: &LogicalPlan,
    accum: &mut BTreeSet<String>,
) -> Result<()> {
    match expr {
        LogicalExpr::Column(name) => {
            accum.insert(name.clone());
        }
        LogicalExpr::ColumnIndex(i) => {
            accum.insert(input.schema()?.field(*i).name().clone());
        }
        LogicalExpr::Binary { lhs, op: _, rhs } => {
            extract_columns(lhs, input, accum)?;
            extract_columns(rhs, input, accum)?;
        }
        LogicalExpr::Alias { expr, alias: _ } => extract_columns(expr, input, accum)?,
        LogicalExpr::Aggregate { name: _, expr } => extract_columns(expr, input, accum)?,
        LogicalExpr::Cast {
            expr: _,
            data_type: _,
        } => {}
        LogicalExpr::Literal(_literal) => {}
    };
    Ok(())
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use insta::assert_snapshot;

    use crate::{optimizer::OptimizerRule, test::plan};

    #[test]
    fn test_projection_push_down() -> Result<()> {
        let plan = plan("SELECT id, first_name, last_name FROM employee WHERE state = 'CO'")?;
        let plan = OptimizerRule::ProjectionPushDown.optimize(&plan)?;
        assert_snapshot!(plan.to_string(), @"
        Projection: #id, #first_name, #last_name
          Filter: #state = 'CO'
            Projection: #id, #first_name, #last_name, #state
              Scan: employee; projection=[id,first_name,last_name,state]
        ");
        Ok(())
    }
}
