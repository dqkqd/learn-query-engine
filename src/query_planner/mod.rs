use std::sync::Arc;

use anyhow::{Result, bail};
use arrow_schema::{Field, Schema};

use crate::{
    logical_plan::{
        LogicalPlan,
        expr::{BinaryOp, Literal, LogicalExpr},
    },
    physical_plan::{
        HashAggregrateExec, PhysicalPlan, ProjectionExec, ScanExec, SelectionExec,
        aggregate::PhysicalAggregateExpr,
        expr::{PhysicalBinaryExpr, PhysicalBinaryOp, PhysicalExpr, PhysicalLiteralExpr},
    },
};

pub fn create_physical_plan(plan: &LogicalPlan) -> Result<PhysicalPlan> {
    let schema = plan.schema()?;
    let plan = match plan {
        LogicalPlan::Scan(scan) => PhysicalPlan::Scan(ScanExec {
            data_source: scan.data_source.clone(),
            projection: scan.projection.clone(),
        }),
        LogicalPlan::Selection(selection) => {
            let input = create_physical_plan(&selection.input)?;
            let filter_expr = create_physical_expr(&selection.expr, &selection.input)?;
            PhysicalPlan::Selection(SelectionExec {
                input: Box::new(input),
                expr: filter_expr,
            })
        }
        LogicalPlan::Projection(projection) => {
            let input = create_physical_plan(&projection.input)?;
            let expr: Result<Vec<PhysicalExpr>> = projection
                .expr
                .iter()
                .map(|v| create_physical_expr(v, &projection.input))
                .collect();
            let expr = expr?;
            let fields: Result<Vec<Field>> = projection
                .expr
                .iter()
                .map(|e| e.to_field(&projection.input))
                .collect();
            let fields = fields?;
            let schema = Schema::new(fields);
            PhysicalPlan::Projection(ProjectionExec {
                schema: Arc::new(schema),
                input: Box::new(input),
                expr,
            })
        }
        LogicalPlan::Aggregate(aggregate) => {
            let input = create_physical_plan(&aggregate.input)?;
            let group_expr: Result<Vec<PhysicalExpr>> = aggregate
                .group_expr
                .iter()
                .map(|e| create_physical_expr(e, &aggregate.input))
                .collect();
            let group_expr = group_expr?;
            let aggregate_expr: Result<Vec<PhysicalAggregateExpr>> = aggregate
                .aggregate_expr
                .iter()
                .map(|e| match e.as_ref() {
                    LogicalExpr::Aggregate { name, expr } => {
                        let expr = create_physical_expr(expr, &aggregate.input)?;
                        let expr = Box::new(expr);
                        let agg_expr = match name.to_lowercase().as_str() {
                            "max" => PhysicalAggregateExpr::Max(expr),
                            "sum" => PhysicalAggregateExpr::Sum(expr),
                            _ => bail!("Unsupported physical aggregate function: `{}`", name),
                        };
                        Ok(agg_expr)
                    }
                    e => bail!("Invalid expression for aggregation in query planner: {}", e),
                })
                .collect();
            let aggregate_expr = aggregate_expr?;
            PhysicalPlan::HashAggregate(HashAggregrateExec {
                schema,
                input: Box::new(input),
                group_expr,
                aggregate_expr,
            })
        }
        LogicalPlan::Join(_join) => todo!(),
    };
    Ok(plan)
}

pub fn create_physical_expr(expr: &LogicalExpr, input: &LogicalPlan) -> Result<PhysicalExpr> {
    let expr = match expr {
        LogicalExpr::Column(name) => {
            let index = input.schema()?.index_of(name)?;
            PhysicalExpr::Column(index)
        }
        LogicalExpr::ColumnIndex(index) => PhysicalExpr::Column(*index),
        LogicalExpr::Literal(literal) => match literal {
            Literal::String(v) => PhysicalExpr::Literal(PhysicalLiteralExpr::String(v.clone())),
            Literal::Long(v) => PhysicalExpr::Literal(PhysicalLiteralExpr::Long(*v)),
            Literal::Double(v) => PhysicalExpr::Literal(PhysicalLiteralExpr::Double(*v)),
        },
        LogicalExpr::Binary { lhs, op, rhs } => {
            let lhs = create_physical_expr(lhs, input)?;
            let rhs = create_physical_expr(rhs, input)?;
            let op = match op {
                BinaryOp::Eq => PhysicalBinaryOp::Eq,
                BinaryOp::Neq => todo!(),
                BinaryOp::Gt => todo!(),
                BinaryOp::GtEq => todo!(),
                BinaryOp::Lt => todo!(),
                BinaryOp::LtEq => todo!(),
                BinaryOp::Plus => PhysicalBinaryOp::Add,
                BinaryOp::Minus => PhysicalBinaryOp::Sub,
                BinaryOp::Multiply => PhysicalBinaryOp::Mul,
                BinaryOp::Divide => PhysicalBinaryOp::Div,
                BinaryOp::As => todo!(),
                BinaryOp::And => todo!(),
            };
            PhysicalExpr::Binary(PhysicalBinaryExpr {
                lhs: Arc::new(lhs),
                op,
                rhs: Arc::new(rhs),
            })
        }
        LogicalExpr::Aggregate { name: _, expr: _ } => todo!(),
        LogicalExpr::Alias { expr, alias: _ } => create_physical_expr(expr, input)?,
        LogicalExpr::Cast {
            expr: _,
            data_type: _,
        } => todo!(),
    };
    Ok(expr)
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use insta::assert_snapshot;

    use crate::{query_planner::create_physical_plan, test::plan};

    #[test]
    fn plan_aggregate_query() -> Result<()> {
        let logical_plan = plan("SELECT state, SUM(salary) FROM employee GROUP BY state")?;
        let physical_plan = create_physical_plan(&logical_plan)?;
        assert_snapshot!(physical_plan.to_string(), @"
        ProjectionExec: #0, #1
          HashAggregrateExec: group_expr=[#3], aggregate_expr=[SUM(#5)]
            ScanExec: [(id,Int64),(first_name,Utf8),(last_name,Utf8),(state,Utf8),(job_title,Utf8),(salary,Int64)]; projection=None
        ");
        Ok(())
    }
}
