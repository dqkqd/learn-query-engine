use std::sync::Arc;

use anyhow::{Result, bail};
use arrow_schema::{ArrowError, Field, Schema};

use crate::{
    logical_plan::{
        LogicalPlan,
        expr::{BinaryOp, Literal, LogicalExpr},
    },
    physical_plan::{
        HashAggregrateExec, HashJoinExec, PhysicalPlan, ProjectionExec, ScanExec, SelectionExec,
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
        LogicalPlan::Join(join) => {
            let lhs = create_physical_plan(&join.left)?;
            let rhs = create_physical_plan(&join.right)?;

            let left_schema = join.left.schema()?;
            let lhs_keys = join
                .on
                .iter()
                .map(|(lhs_key, _)| left_schema.index_of(lhs_key))
                .collect::<Result<Vec<_>, ArrowError>>();
            let lhs_keys = lhs_keys?;

            let right_schema = join.right.schema()?;
            let rhs_key = join
                .on
                .iter()
                .map(|(_, rhs_key)| right_schema.index_of(rhs_key))
                .collect::<Result<Vec<_>, ArrowError>>();
            let rhs_keys = rhs_key?;

            let right_columns_to_include = (0..right_schema.fields().len())
                .filter(|index| {
                    let field = right_schema.field(*index);
                    left_schema.field_with_name(field.name()).is_err()
                })
                .collect::<Vec<_>>();

            PhysicalPlan::Join(HashJoinExec {
                schema,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                join_type: join.join_type.clone(),
                lhs_keys,
                rhs_keys,
                right_columns_to_include,
            })
        }
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
    use arrow::util::pretty::pretty_format_batches;
    use insta::assert_snapshot;

    use crate::{
        query_planner::create_physical_plan,
        test::{execute_physical_plan, plan},
    };

    #[test]
    fn plan_aggregate_query() -> Result<()> {
        let logical_plan = plan("SELECT state, SUM(salary) FROM employee GROUP BY state")?;
        let physical_plan = create_physical_plan(&logical_plan)?;
        assert_snapshot!(physical_plan.to_string(), @"
        ProjectionExec: #0, #1
          HashAggregrateExec: group_expr=[#3], aggregate_expr=[SUM(#5)]
            ScanExec: [(id,Int64),(first_name,Utf8),(last_name,Utf8),(state,Utf8),(job_title,Utf8),(salary,Int64)]; projection=None
        ");
        let batches = execute_physical_plan(physical_plan)?;
        assert_snapshot!(pretty_format_batches(&batches)?, @"
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
