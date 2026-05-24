use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use anyhow::{Context, Result, bail};
use arrow_schema::DataType;

use crate::{
    dataframe::{DataFrame, col, col_index},
    logical_plan::expr::{BinaryOp, Literal, LogicalExpr},
    sql::expr::{Select, SqlExpr, SqlIdentifier},
};

pub fn create_dataframe(
    sql_expr: SqlExpr,
    tables: &HashMap<String, DataFrame>,
) -> Result<DataFrame> {
    let SqlExpr::Select(select) = sql_expr else {
        bail!("Not a select statement: {:?}", sql_expr)
    };

    let df = tables
        .get(&select.table_name.0)
        .with_context(|| format!("No table name `{}` in dataframe list", select.table_name.0))?
        .clone();

    let projection: Result<Vec<Arc<LogicalExpr>>> =
        select.projection.iter().map(create_logical_expr).collect();
    let projection_expr = projection?;

    let aggregate_count = projection_expr
        .iter()
        .filter(|e| matches!(e.as_ref(), LogicalExpr::Aggregate { .. }))
        .count();

    if aggregate_count == 0 {
        return plan_non_aggregate_query(select, df, projection_expr);
    }

    let group_by: Result<Vec<Arc<LogicalExpr>>> =
        select.group_by.iter().map(create_logical_expr).collect();
    let group_by = group_by?;

    let mut aggregate = vec![];
    let mut projection = vec![];
    let num_group = select.group_by.len();
    let mut group_count = 0;
    // if the column is referenced in the group by and aggregate,
    // replace it in the projection
    for expr in &projection_expr {
        match expr.as_ref() {
            LogicalExpr::Aggregate { .. } => {
                projection.push(col_index(num_group + aggregate.len()));
                aggregate.push(expr.clone())
            }
            LogicalExpr::Alias { expr: e, alias } => {
                if matches!(e.as_ref(), LogicalExpr::Aggregate { .. }) {
                    bail!(
                        "Alias in aggregate query must wrap an aggregate expression, found `{}`",
                        expr
                    )
                }
                projection.push(col_index(num_group + aggregate.len()).alias(alias));
                aggregate.push(e.clone());
            }
            // group by columns
            _ => {
                projection.push(col_index(group_count));
                group_count += 1;
            }
        }
    }

    let df = match select.filter {
        Some(filter) => {
            let projection_expr_without_aggregates = projection_expr
                .iter()
                .filter(|p| !matches!(p.as_ref(), LogicalExpr::Aggregate { .. }))
                .map(Arc::clone)
                .collect::<Vec<_>>();
            let column_names_in_projection_without_aggregates =
                get_referenced_columns(&projection_expr_without_aggregates);
            let column_names_in_aggregates = get_referenced_columns(&aggregate);
            let column_names_in_filter = get_valid_referened_columns(&filter, &df)?;
            let all_required_columns = &(&column_names_in_projection_without_aggregates
                | &column_names_in_filter)
                | &column_names_in_aggregates;

            let missing = all_required_columns
                .difference(&column_names_in_projection_without_aggregates)
                .collect::<Vec<_>>();

            if missing.is_empty() {
                let df = df.project(projection_expr_without_aggregates);
                let filter = create_logical_expr(&filter)?;
                df.filter(filter)
            } else {
                let full_projection: Vec<Arc<LogicalExpr>> = projection_expr_without_aggregates
                    .iter()
                    .cloned()
                    .chain(missing.iter().map(col))
                    .collect();
                let filter = create_logical_expr(&filter)?;
                df.project(full_projection).filter(filter)
            }
        }
        None => df,
    };

    let df = df.aggregate(group_by, aggregate);
    let df = df.project(projection);

    let Some(having) = select.having else {
        return Ok(df);
    };

    let having = create_logical_expr(&having)?;
    let df = df.filter(having);

    Ok(df)
}

fn plan_non_aggregate_query(
    select: Select,
    df: DataFrame,
    projection: Vec<Arc<LogicalExpr>>,
) -> Result<DataFrame> {
    // no filter
    let Some(filter) = select.filter else {
        let df = df.project(projection);
        return Ok(df);
    };

    let column_names_in_projection = get_referenced_columns(&projection);
    let column_names_in_filter = get_valid_referened_columns(&filter, &df)?;
    let missing = column_names_in_filter
        .difference(&column_names_in_projection)
        .collect::<Vec<_>>();

    let filter = create_logical_expr(&filter)?;

    // everything in filter exists in projection
    if missing.is_empty() {
        let df = df.project(projection).filter(filter);
        return Ok(df);
    }

    // filter contains columns that projection doesn't.
    let full_projection: Vec<Arc<LogicalExpr>> = projection
        .iter()
        .cloned()
        .chain(missing.iter().map(col))
        .collect();
    let df = df
        // 1. Add missing columns
        .project(full_projection)
        // 2. Filter
        .filter(filter)
        // 3. Drop redundant columns
        .project(projection);

    Ok(df)
}

fn get_referenced_columns(exprs: &[Arc<LogicalExpr>]) -> BTreeSet<String> {
    let mut accumulator = BTreeSet::new();
    for expr in exprs {
        visit(expr, &mut accumulator);
    }
    accumulator
}

fn get_valid_referened_columns(expr: &SqlExpr, table: &DataFrame) -> Result<BTreeSet<String>> {
    let logical_expr = create_logical_expr(expr)?;
    let referenced_columns = get_referenced_columns(&[logical_expr]);
    let valid_column_names: BTreeSet<String> = table
        .plan()
        .schema()?
        .fields
        .iter()
        .map(|f| f.name().clone())
        .collect();
    let referenced_columns: BTreeSet<String> = referenced_columns
        .intersection(&valid_column_names)
        .cloned()
        .collect();
    Ok(referenced_columns)
}

fn visit(expr: &LogicalExpr, accumulator: &mut BTreeSet<String>) {
    match expr {
        LogicalExpr::Column(c) => {
            accumulator.insert(c.clone());
        }
        LogicalExpr::Binary { lhs, op: _, rhs } => {
            visit(lhs, accumulator);
            visit(rhs, accumulator);
        }
        LogicalExpr::Aggregate { name: _, expr } => visit(expr, accumulator),
        LogicalExpr::Alias { expr, alias: _ } => visit(expr, accumulator),
        _ => (),
    }
}

fn create_logical_expr(expr: &SqlExpr) -> Result<Arc<LogicalExpr>> {
    let logical_expr = match expr {
        SqlExpr::Indentifier(sql_identifier) => LogicalExpr::Column(sql_identifier.0.clone()),
        SqlExpr::String(value) => LogicalExpr::Literal(Literal::String(value.clone())),
        SqlExpr::Long(value) => LogicalExpr::Literal(Literal::Long(*value)),
        SqlExpr::Double(value) => LogicalExpr::Literal(Literal::Double(*value)),
        SqlExpr::BinaryExpr { lhs, op, rhs } => {
            let lhs = create_logical_expr(lhs)?;
            let rhs = create_logical_expr(rhs)?;
            LogicalExpr::Binary {
                lhs,
                op: BinaryOp::try_from(op.as_str())?,
                rhs,
            }
        }
        SqlExpr::Alias { expr, alias } => {
            let expr = create_logical_expr(expr)?;
            LogicalExpr::Alias {
                expr,
                alias: alias.0.clone(),
            }
        }
        SqlExpr::Function { id, args } => match id.to_uppercase().as_str() {
            "MIN" | "MAX" | "SUM" | "AVG" => {
                let first = args
                    .first()
                    .with_context(|| format!("{} requires an argument", id.to_uppercase()))?;
                let expr = create_logical_expr(first)?;
                LogicalExpr::Aggregate {
                    name: id.to_uppercase(),
                    expr,
                }
            }
            "COUNT" => {
                let first = args
                    .first()
                    .with_context(|| format!("{} requires an argument", id.to_uppercase()))?;
                if first == &SqlExpr::Indentifier(SqlIdentifier("*".to_string())) {
                    LogicalExpr::Aggregate {
                        name: id.to_uppercase(),
                        expr: Arc::new(LogicalExpr::Literal(Literal::Long(1))),
                    }
                } else {
                    LogicalExpr::Aggregate {
                        name: id.to_uppercase(),
                        expr: create_logical_expr(expr)?,
                    }
                }
            }
            c => unimplemented!("{}", c),
        },
        SqlExpr::Cast { expr, data_type } => {
            let data_type = parse_data_type(&data_type.0)?;
            let expr = create_logical_expr(expr)?;
            LogicalExpr::Cast { expr, data_type }
        }
        SqlExpr::Sort { expr: _, asc: _ } => todo!(),
        SqlExpr::Select(_select) => todo!(),
    };
    Ok(Arc::new(logical_expr))
}

fn parse_data_type(data_type: &str) -> Result<DataType> {
    match data_type.to_lowercase().as_str() {
        "double" => Ok(DataType::Float64),
        _ => bail!("Unsupported CAST data type `{}`", data_type),
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use insta::assert_snapshot;

    use crate::test::plan;

    #[test]
    fn simple_select() -> Result<()> {
        let plan = plan("SELECT state FROM employee")?;
        assert_snapshot!(plan.to_string(), @"
        Projection: #state
          Scan: employee; projection=None
        ");
        Ok(())
    }

    #[test]
    fn select_with_filter() -> Result<()> {
        let plan = plan("SELECT state FROM employee WHERE state = 'CA'")?;
        assert_snapshot!(plan.to_string(), @"
        Filter: #state = 'CA'
          Projection: #state
            Scan: employee; projection=None
        ");
        Ok(())
    }

    #[test]
    fn select_with_filter_not_in_projection() -> Result<()> {
        let plan = plan("SELECT last_name FROM employee WHERE state = 'CA'")?;
        assert_snapshot!(plan.to_string(), @"
        Projection: #last_name
          Filter: #state = 'CA'
            Projection: #last_name, #state
              Scan: employee; projection=None
        ");
        Ok(())
    }

    #[test]
    fn select_filter_on_projection() -> Result<()> {
        let plan = plan("SELECT last_name AS foo FROM employee WHERE foo = 'Einstein'")?;
        assert_snapshot!(plan.to_string(), @"
        Filter: #foo = 'Einstein'
          Projection: #last_name as foo
            Scan: employee; projection=None
        ");
        Ok(())
    }

    #[test]
    fn select_filter_on_projection_and_not() -> Result<()> {
        let plan = plan(
            r#"
SELECT last_name AS foo
FROM employee 
WHERE foo = 'Einstein' AND state = 'CA'
"#,
        )?;
        assert_snapshot!(plan.to_string(), @"
        Projection: #last_name as foo
          Filter: #foo = 'Einstein' AND #state = 'CA'
            Projection: #last_name as foo, #state
              Scan: employee; projection=None
        ");
        Ok(())
    }

    #[test]
    fn plan_aggregate_query() -> Result<()> {
        let plan = plan("SELECT state, MAX(salary) FROM employee GROUP BY state")?;
        assert_snapshot!(plan.to_string(), @"
        Projection: #0, #1
          Aggregate: group_expr=[#state], aggregate_expr=[MAX(#salary)]
            Scan: employee; projection=None
        ");
        Ok(())
    }

    #[test]
    fn plan_aggregate_query_with_having() -> Result<()> {
        let plan =
            plan("SELECT state, MAX(salary) FROM employee GROUP BY state HAVING MAX(salary) > 10")?;
        assert_snapshot!(plan.to_string(), @"
        Filter: MAX(#salary) > 10
          Projection: #0, #1
            Aggregate: group_expr=[#state], aggregate_expr=[MAX(#salary)]
              Scan: employee; projection=None
        ");
        Ok(())
    }

    #[test]
    fn plan_aggregate_query_aggr_first() -> Result<()> {
        let plan = plan("SELECT MAX(salary), state FROM employee GROUP BY state")?;
        assert_snapshot!(plan.to_string(), @"
        Projection: #1, #0
          Aggregate: group_expr=[#state], aggregate_expr=[MAX(#salary)]
            Scan: employee; projection=None
        ");
        Ok(())
    }

    #[test]
    fn plan_aggregate_query_with_filter() -> Result<()> {
        let plan =
            plan("SELECT state, MAX(salary) FROM employee WHERE salary > 50000 GROUP BY state")?;
        assert_snapshot!(plan.to_string(), @"
        Projection: #0, #1
          Aggregate: group_expr=[#state], aggregate_expr=[MAX(#salary)]
            Filter: #salary > 50000
              Projection: #state, #salary
                Scan: employee; projection=None
        ");
        Ok(())
    }

    #[test]
    fn plan_aggregate_query_with_cast() -> Result<()> {
        let plan = plan("SELECT state, MAX(CAST(salary AS double)) FROM employee GROUP BY state")?;
        assert_snapshot!(plan.to_string(), @"
        Projection: #0, #1
          Aggregate: group_expr=[#state], aggregate_expr=[MAX(#salary as Float64)]
            Scan: employee; projection=None
        ");
        Ok(())
    }
}
