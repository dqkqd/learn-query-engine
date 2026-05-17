use anyhow::{Result, bail};
use arrow_schema::{DataType, Field};
use std::{fmt::Display, sync::Arc};

use crate::logical_plan::LogicalPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogicalExpr {
    Column(String),
    ColumnIndex(usize),
    Literal(Literal),
    Binary {
        lhs: Arc<LogicalExpr>,
        op: BinaryOp,
        rhs: Arc<LogicalExpr>,
    },
    Aggregate {
        name: String,
        expr: Arc<LogicalExpr>,
    },
    Alias {
        expr: Arc<LogicalExpr>,
        alias: String,
    },
    Cast {
        expr: Arc<LogicalExpr>,
        data_type: DataType,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    String(String),
    Long(i64),
    Double(f64),
}

impl Eq for Literal {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryOp {
    Eq,
    Neq,
    Gt,
    GtEq,
    Lt,
    LtEq,

    Plus,
    Minus,
    Multiply,
    Divide,
    As,
    And,
}

impl LogicalExpr {
    pub fn to_field(&self, input: &LogicalPlan) -> Result<Field> {
        let schema = input.schema()?;
        match self {
            LogicalExpr::Column(name) => {
                let field = schema.field_with_name(name)?;
                Ok(field.clone())
            }
            LogicalExpr::ColumnIndex(i) => Ok(schema.field(*i).clone()),
            LogicalExpr::Literal(literal) => match literal {
                Literal::String(s) => Ok(Field::new(s, DataType::Utf8, true)),
                Literal::Long(l) => Ok(Field::new(l.to_string(), DataType::Int64, true)),
                Literal::Double(d) => Ok(Field::new(d.to_string(), DataType::Float64, true)),
            },
            LogicalExpr::Binary {
                lhs: left,
                op,
                rhs: right,
            } => {
                let left_field = left.to_field(input)?;
                let right_field = right.to_field(input)?;
                if left_field.data_type() != right_field.data_type() {
                    unimplemented!("handle mismatch data type");
                }
                Ok(Field::new(
                    op.to_string(),
                    left_field.data_type().clone(),
                    left_field.is_nullable() || right_field.is_nullable(),
                ))
            }
            LogicalExpr::Aggregate { name, expr } => Ok(expr.to_field(input)?.with_name(name)),
            LogicalExpr::Alias { expr, alias } => Ok(expr.to_field(input)?.with_name(alias)),
            LogicalExpr::Cast { expr, data_type } => {
                Ok(expr.to_field(input)?.with_data_type(data_type.clone()))
            }
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

impl Display for BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinaryOp::Eq => write!(f, "="),
            BinaryOp::Neq => write!(f, "!="),
            BinaryOp::Gt => write!(f, ">"),
            BinaryOp::GtEq => write!(f, ">="),
            BinaryOp::Lt => write!(f, "<"),
            BinaryOp::LtEq => write!(f, "<="),
            BinaryOp::Plus => write!(f, "+"),
            BinaryOp::Minus => write!(f, "-"),
            BinaryOp::Multiply => write!(f, "*"),
            BinaryOp::Divide => write!(f, "/"),
            BinaryOp::As => write!(f, "AS"),
            BinaryOp::And => write!(f, "AND"),
        }
    }
}

impl TryFrom<&str> for BinaryOp {
    type Error = anyhow::Error;

    fn try_from(op: &str) -> Result<Self, Self::Error> {
        let op = match op.to_lowercase().as_str() {
            "=" => BinaryOp::Eq,
            "!=" => BinaryOp::Neq,
            ">" => BinaryOp::Gt,
            ">=" => BinaryOp::GtEq,
            "<" => BinaryOp::Lt,
            "<=" => BinaryOp::LtEq,
            "+" => BinaryOp::Plus,
            "-" => BinaryOp::Minus,
            "*" => BinaryOp::Multiply,
            "/" => BinaryOp::Divide,
            "as" => BinaryOp::As,
            "and" => BinaryOp::And,
            _ => bail!("Invalid binary op: `{}`", op),
        };
        Ok(op)
    }
}

impl Display for LogicalExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogicalExpr::Column(name) => write!(f, "#{}", name),
            LogicalExpr::ColumnIndex(i) => write!(f, "#{}", i),
            LogicalExpr::Literal(literal) => write!(f, "{}", literal),
            LogicalExpr::Binary {
                lhs: left,
                op,
                rhs: right,
            } => write!(f, "{} {} {}", left, op, right),
            LogicalExpr::Aggregate { name, expr } => write!(f, "{}({})", name, expr),
            LogicalExpr::Alias { expr, alias } => write!(f, "{} as {}", expr, alias),
            LogicalExpr::Cast { expr, data_type } => write!(f, "{} as {}", expr, data_type),
        }
    }
}
