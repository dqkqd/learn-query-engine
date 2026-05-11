mod expr;

use anyhow::Result;
use arrow::datatypes::Schema;
use arrow_schema::Field;
use std::{fmt::Display, sync::Arc};

use crate::{data_source::DataSource, logical_plan::expr::LogicalExpr};

pub enum LogicalPlan {
    Scan(Scan),
    Selection(Selection),
    Projection(Projection),
    Aggregate(Aggregate),
    Join(Join),
}

pub struct Scan {
    path: String,
    data_source: Box<dyn DataSource>,
    projection: Vec<String>,
}

pub struct Selection {
    input: Box<LogicalPlan>,
    expr: LogicalExpr,
}

pub struct Projection {
    input: Box<LogicalPlan>,
    expr: Vec<LogicalExpr>,
}

pub struct Aggregate {
    input: Box<LogicalPlan>,
    group_expr: Vec<LogicalExpr>,
    aggregate_expr: Vec<LogicalExpr>,
}

pub struct Join {
    left: Box<LogicalPlan>,
    right: Box<LogicalPlan>,
    join_type: JoinType,
    on: Vec<(String, String)>,
}

pub enum JoinType {
    Inner,
    Left,
    Right,
}

impl LogicalPlan {
    pub fn schema(&self) -> Result<Arc<Schema>> {
        match self {
            LogicalPlan::Scan(scan) => scan.schema(),
            LogicalPlan::Selection(selection) => selection.schema(),
            LogicalPlan::Projection(projection) => projection.schema(),
            LogicalPlan::Aggregate(aggregate) => aggregate.schema(),
            LogicalPlan::Join(join) => join.schema(),
        }
    }

    pub fn children(&self) -> Vec<&LogicalPlan> {
        match self {
            LogicalPlan::Scan(scan) => scan.children(),
            LogicalPlan::Selection(selection) => selection.children(),
            LogicalPlan::Projection(projection) => projection.children(),
            LogicalPlan::Aggregate(aggregate) => aggregate.children(),
            LogicalPlan::Join(join) => join.children(),
        }
    }
}

impl Scan {
    fn schema(&self) -> Result<Arc<Schema>> {
        let schema = self.data_source.schema();
        if self.projection.is_empty() {
            Ok(schema)
        } else {
            let mut field_ids = Vec::with_capacity(self.projection.len());
            for name in &self.projection {
                let field_id = schema.index_of(name)?;
                field_ids.push(field_id);
            }
            let schema = schema.project(&field_ids)?;
            Ok(Arc::new(schema))
        }
    }

    fn children(&self) -> Vec<&LogicalPlan> {
        vec![]
    }
}

impl Selection {
    fn schema(&self) -> Result<Arc<Schema>> {
        self.input.schema()
    }

    fn children(&self) -> Vec<&LogicalPlan> {
        vec![&self.input]
    }
}

impl Projection {
    fn schema(&self) -> Result<Arc<Schema>> {
        let fields: Result<Vec<Field>> =
            self.expr.iter().map(|e| e.to_field(&self.input)).collect();
        let fields = fields?;
        let schema = Schema::new(fields);
        Ok(Arc::new(schema))
    }

    fn children(&self) -> Vec<&LogicalPlan> {
        vec![&self.input]
    }
}

impl Aggregate {
    fn schema(&self) -> Result<Arc<Schema>> {
        let groups: Result<Vec<Field>> = self
            .group_expr
            .iter()
            .map(|e| e.to_field(&self.input))
            .collect();
        let aggregate: Result<Vec<Field>> = self
            .aggregate_expr
            .iter()
            .map(|e| e.to_field(&self.input))
            .collect();
        let fields = [groups?, aggregate?].concat();
        let schema = Schema::new(fields);
        Ok(Arc::new(schema))
    }

    fn children(&self) -> Vec<&LogicalPlan> {
        vec![&self.input]
    }
}

impl Join {
    fn schema(&self) -> Result<Arc<Schema>> {
        todo!()
    }

    fn children(&self) -> Vec<&LogicalPlan> {
        vec![&self.left, &self.right]
    }
}

struct LogicalPlanDisplay<'a> {
    indent: usize,
    plan: &'a LogicalPlan,
}

impl<'a> Display for LogicalPlanDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let indent = "  ".repeat(self.indent);
        write!(f, "{}", indent)?;
        match self.plan {
            LogicalPlan::Scan(scan) => writeln!(f, "{}", scan)?,
            LogicalPlan::Selection(selection) => writeln!(f, "{}", selection)?,
            LogicalPlan::Projection(projection) => writeln!(f, "{}", projection)?,
            LogicalPlan::Aggregate(aggregate) => writeln!(f, "{}", aggregate)?,
            LogicalPlan::Join(_join) => todo!(),
        }
        for c in self.plan.children() {
            LogicalPlanDisplay {
                indent: self.indent + 1,
                plan: c,
            }
            .fmt(f)?;
        }
        Ok(())
    }
}

impl Display for LogicalPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        LogicalPlanDisplay {
            indent: 0,
            plan: self,
        }
        .fmt(f)
    }
}

impl Display for Scan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.projection.is_empty() {
            write!(f, "Scan: {}; projection=None", self.path)
        } else {
            write!(
                f,
                "Scan: {}; projection=[{}]",
                self.path,
                self.projection.join(",")
            )
        }
    }
}

impl Display for Selection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Filter: {}", self.expr)
    }
}

impl Display for Projection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fields: Vec<String> = self.expr.iter().map(|e| e.to_string()).collect();
        write!(f, "Projection: {}", fields.join(", "))
    }
}

impl Display for Aggregate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let groups: Vec<String> = self.group_expr.iter().map(|e| e.to_string()).collect();
        let aggregate: Vec<String> = self.aggregate_expr.iter().map(|e| e.to_string()).collect();
        write!(
            f,
            "Aggregate: group_expr={}, aggregate_expr={}",
            groups.join(", "),
            aggregate.join(", ")
        )
    }
}

impl Display for JoinType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JoinType::Inner => write!(f, "inner"),
            JoinType::Left => write!(f, "left"),
            JoinType::Right => write!(f, "right"),
        }
    }
}

impl Display for Join {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let on: Vec<String> = self
            .on
            .iter()
            .map(|(l, r)| format!("{}={}", l, r))
            .collect();
        write!(f, "Join: type={}, on={}", self.join_type, on.join(", "))
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use arrow::array::record_batch;
    use insta::assert_snapshot;

    use crate::{
        data_source::{DataSource, memory::MemoryDataSource},
        logical_plan::{
            LogicalPlan, Projection, Scan, Selection,
            expr::{Literal, LogicalExpr},
        },
    };

    fn data_source() -> Result<impl DataSource> {
        let batch = record_batch!(
            ("a", Int32, [1, 2, 3, 4, 5, 6, 7, 8]),
            (
                "b",
                Utf8,
                [
                    "one", "two", "three", "four", "five", "six", "seven", "eight"
                ]
            )
        )?;
        let schema = batch.schema();
        Ok(MemoryDataSource::new(schema, vec![batch]))
    }

    #[test]
    fn test_scan() -> Result<()> {
        let plan = LogicalPlan::Scan(Scan {
            path: "users".to_string(),
            data_source: Box::new(data_source()?),
            projection: vec!["name".to_string()],
        });
        assert_snapshot!(plan.to_string(), @"Scan: users; projection=[name]");
        Ok(())
    }

    #[test]
    fn test_selection() -> Result<()> {
        let scan = LogicalPlan::Scan(Scan {
            path: "users".to_string(),
            data_source: Box::new(data_source()?),
            projection: vec!["name".to_string()],
        });
        let plan = LogicalPlan::Selection(Selection {
            input: Box::new(scan),
            expr: LogicalExpr::Binary {
                name: "one".to_string(),
                left: Box::new(LogicalExpr::Column("name".to_string())),
                op: "=".to_string(),
                right: Box::new(LogicalExpr::Literal(Literal::String("Alice".to_string()))),
            },
        });
        assert_snapshot!(plan.to_string(), @"
        Filter: #name = 'Alice'
          Scan: users; projection=[name]
        ");
        Ok(())
    }

    #[test]
    fn test_compose() -> Result<()> {
        let scan = LogicalPlan::Scan(Scan {
            path: "employees".to_string(),
            data_source: Box::new(data_source()?),
            projection: vec![],
        });

        let filter = LogicalPlan::Selection(Selection {
            input: Box::new(scan),
            expr: LogicalExpr::Binary {
                name: "Eq".to_string(),
                left: Box::new(LogicalExpr::Column("department".to_string())),
                op: "=".to_string(),
                right: Box::new(LogicalExpr::Literal(Literal::String(
                    "Engineering".to_string(),
                ))),
            },
        });

        let project = LogicalPlan::Projection(Projection {
            input: Box::new(filter),
            expr: vec![
                LogicalExpr::Column("name".to_string()),
                LogicalExpr::Alias {
                    expr: Box::new(LogicalExpr::Binary {
                        name: "multiply".to_string(),
                        left: Box::new(LogicalExpr::Column("salary".to_string())),
                        op: "*".to_string(),
                        right: Box::new(LogicalExpr::Literal(Literal::Double(1.1))),
                    }),
                    alias: "new_salary".to_string(),
                },
            ],
        });

        assert_snapshot!(project.to_string(), @"
        Projection: #name, #salary * 1.1 as new_salary
          Filter: #department = 'Engineering'
            Scan: employees; projection=None
        ");
        Ok(())
    }
}
