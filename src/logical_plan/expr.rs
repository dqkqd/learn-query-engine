use anyhow::Result;
use arrow_schema::{DataType, Field};
use std::fmt::Display;

use crate::logical_plan::LogicalPlan;

pub enum LogicalExpr {
    Column(String),
    Literal(Literal),
    Binary {
        name: String,
        left: Box<LogicalExpr>,
        op: String,
        right: Box<LogicalExpr>,
    },
    Aggregate {
        name: String,
        expr: Box<LogicalExpr>,
    },
    Alias {
        expr: Box<LogicalExpr>,
        alias: String,
    },
}

pub enum Literal {
    String(String),
    Long(u64),
    Double(f64),
}

impl LogicalExpr {
    pub fn to_field(&self, input: &LogicalPlan) -> Result<Field> {
        let schema = input.schema()?;
        match self {
            LogicalExpr::Column(name) => {
                let field = schema.field_with_name(name)?;
                Ok(field.clone())
            }
            LogicalExpr::Literal(literal) => match literal {
                Literal::String(s) => Ok(Field::new(s, DataType::Utf8, true)),
                Literal::Long(l) => Ok(Field::new(l.to_string(), DataType::Int64, true)),
                Literal::Double(d) => Ok(Field::new(d.to_string(), DataType::Float64, true)),
            },
            LogicalExpr::Binary {
                name,
                left,
                op: _,
                right,
            } => {
                let left_field = left.to_field(input)?;
                let right_field = right.to_field(input)?;
                if left_field.data_type() != right_field.data_type() {
                    unimplemented!("handle mismatch data type");
                }
                Ok(Field::new(
                    name,
                    left_field.data_type().clone(),
                    left_field.is_nullable() || right_field.is_nullable(),
                ))
            }
            LogicalExpr::Aggregate { name, expr } => Ok(expr.to_field(input)?.with_name(name)),
            LogicalExpr::Alias { expr, alias } => Ok(expr.to_field(input)?.with_name(alias)),
        }
    }
}

impl Display for Literal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Literal::String(s) => write!(f, "'{}'", s),
            Literal::Long(l) => write!(f, "{}", l),
            Literal::Double(d) => write!(f, "{}", d),
        }
    }
}

impl Display for LogicalExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogicalExpr::Column(name) => write!(f, "#{}", name),
            LogicalExpr::Literal(literal) => write!(f, "{}", literal),
            LogicalExpr::Binary {
                name: _,
                left,
                op,
                right,
            } => write!(f, "{} {} {}", left, op, right),
            LogicalExpr::Aggregate { name, expr } => write!(f, "{}({})", name, expr),
            LogicalExpr::Alias { expr, alias } => write!(f, "{} as {}", expr, alias),
        }
    }
}
