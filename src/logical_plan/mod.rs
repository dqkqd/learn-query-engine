pub mod expr;

use anyhow::Result;
use arrow::datatypes::Schema;
use arrow_schema::Field;
use std::{collections::BTreeSet, fmt::Display, sync::Arc};

use crate::{data_source::DataSource, logical_plan::expr::LogicalExpr, utils::field_ids_by_names};

#[derive(Debug, Clone)]
pub enum LogicalPlan {
    Scan(Scan),
    Selection(Selection),
    Projection(Projection),
    Aggregate(Aggregate),
    Join(Join),
}

#[derive(Debug, Clone)]
pub struct Scan {
    pub path: String,
    pub data_source: DataSource,
    pub projection: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Selection {
    pub input: Box<LogicalPlan>,
    pub expr: Arc<LogicalExpr>,
}

#[derive(Debug, Clone)]
pub struct Projection {
    pub input: Box<LogicalPlan>,
    pub expr: Vec<Arc<LogicalExpr>>,
}

#[derive(Debug, Clone)]
pub struct Aggregate {
    pub input: Box<LogicalPlan>,
    pub group_expr: Vec<Arc<LogicalExpr>>,
    pub aggregate_expr: Vec<Arc<LogicalExpr>>,
}

#[derive(Debug, Clone)]
pub struct Join {
    pub left: Box<LogicalPlan>,
    pub right: Box<LogicalPlan>,
    pub join_type: JoinType,
    pub on: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
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
        let schema = self.data_source.schema()?;
        if self.projection.is_empty() {
            Ok(schema)
        } else {
            let field_ids = field_ids_by_names(&schema, &self.projection)?;
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
        let lhs = self.left.schema()?;
        let lhs = lhs.fields();
        let rhs = self.right.schema()?;
        let rhs = rhs.fields();

        let (lhs, rhs) = match self.join_type {
            JoinType::Inner | JoinType::Left => (lhs, rhs),
            JoinType::Right => (rhs, lhs),
        };

        let lhs_names = lhs.iter().map(|f| f.name()).collect::<BTreeSet<_>>();
        let rhs = rhs.iter().filter(|f| !lhs_names.contains(f.name()));
        let fields = lhs.iter().cloned().chain(rhs.cloned()).collect::<Vec<_>>();
        let schema = Schema::new(fields);
        Ok(Arc::new(schema))
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
            LogicalPlan::Join(join) => write!(f, "{}", join)?,
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
            "Aggregate: group_expr=[{}], aggregate_expr=[{}]",
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
    use std::sync::Arc;

    use anyhow::Result;
    use arrow::array::record_batch;
    use insta::assert_snapshot;

    use crate::{
        data_source::{DataSource, memory::MemoryDataSource},
        logical_plan::{
            LogicalPlan, Projection, Scan, Selection,
            expr::{BinaryOp, Literal, LogicalExpr},
        },
    };

    fn data_source() -> Result<DataSource> {
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
        Ok(DataSource::Memory(MemoryDataSource::new(
            schema,
            vec![batch],
        )))
    }

    #[test]
    fn test_scan() -> Result<()> {
        let plan = LogicalPlan::Scan(Scan {
            path: "users".to_string(),
            data_source: data_source()?,
            projection: vec!["name".to_string()],
        });
        assert_snapshot!(plan.to_string(), @"Scan: users; projection=[name]");
        Ok(())
    }

    #[test]
    fn test_selection() -> Result<()> {
        let scan = LogicalPlan::Scan(Scan {
            path: "users".to_string(),
            data_source: data_source()?,
            projection: vec!["name".to_string()],
        });
        let plan = LogicalPlan::Selection(Selection {
            input: Box::new(scan),
            expr: Arc::new(LogicalExpr::Binary {
                lhs: Arc::new(LogicalExpr::Column("name".to_string())),
                op: BinaryOp::Eq,
                rhs: Arc::new(LogicalExpr::Literal(Literal::String("Alice".to_string()))),
            }),
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
            data_source: data_source()?,
            projection: vec![],
        });

        let filter = LogicalPlan::Selection(Selection {
            input: Box::new(scan),
            expr: Arc::new(LogicalExpr::Binary {
                lhs: Arc::new(LogicalExpr::Column("department".to_string())),
                op: BinaryOp::Eq,
                rhs: Arc::new(LogicalExpr::Literal(Literal::String(
                    "Engineering".to_string(),
                ))),
            }),
        });

        let project = LogicalPlan::Projection(Projection {
            input: Box::new(filter),
            expr: vec![
                Arc::new(LogicalExpr::Column("name".to_string())),
                Arc::new(LogicalExpr::Alias {
                    expr: Arc::new(LogicalExpr::Binary {
                        lhs: Arc::new(LogicalExpr::Column("salary".to_string())),
                        op: BinaryOp::Eq,
                        rhs: Arc::new(LogicalExpr::Literal(Literal::Double(1.1))),
                    }),
                    alias: "new_salary".to_string(),
                }),
            ],
        });

        assert_snapshot!(project.to_string(), @"
        Projection: #name, #salary = 1.1 as new_salary
          Filter: #department = 'Engineering'
            Scan: employees; projection=None
        ");
        Ok(())
    }
}
