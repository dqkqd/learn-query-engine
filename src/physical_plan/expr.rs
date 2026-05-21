use std::{fmt::Display, sync::Arc};

use anyhow::Result;
use arrow::{
    array::{ArrayRef, Float64Array, Int64Array, RecordBatch, StringArray},
    compute::kernels,
};
use arrow_schema::ArrowError;


#[derive(Debug)]
pub enum PhysicalExpr {
    Column(usize),
    Literal(PhysicalLiteralExpr),
    Binary(PhysicalBinaryExpr),
}

#[derive(Debug)]
pub enum PhysicalLiteralExpr {
    String(String),
    Long(i64),
    Double(f64),
}

#[derive(Debug)]
pub struct PhysicalBinaryExpr {
    pub lhs: Arc<PhysicalExpr>,
    pub op: PhysicalBinaryOp,
    pub rhs: Arc<PhysicalExpr>,
}

#[derive(Debug)]
pub enum PhysicalBinaryOp {
    Eq,
    // Neq,
    // Gt,
    // GtEq,
    // Lt,
    // LtEq,
    // And,
    //
    Add,
    Sub,
    Mul,
    Div,
}

impl PhysicalExpr {
    pub fn evaluate(&self, input: &RecordBatch) -> Result<ArrayRef, ArrowError> {
        let array: ArrayRef = match self {
            PhysicalExpr::Column(index) => Arc::clone(input.column(*index)),
            PhysicalExpr::Literal(physical_literal_expr) => match physical_literal_expr {
                PhysicalLiteralExpr::String(s) => {
                    let values = std::iter::repeat_n(Some(s.clone()), input.num_rows())
                        .collect::<StringArray>();
                    Arc::new(values)
                }
                PhysicalLiteralExpr::Long(v) => {
                    let values = Int64Array::from_value(*v, input.num_rows());
                    Arc::new(values)
                }
                PhysicalLiteralExpr::Double(v) => {
                    let values = Float64Array::from_value(*v, input.num_rows());
                    Arc::new(values)
                }
            },
            PhysicalExpr::Binary(physical_binary_expr) => {
                let lhs = physical_binary_expr.lhs.evaluate(input)?;
                let rhs = physical_binary_expr.rhs.evaluate(input)?;
                match physical_binary_expr.op {
                    PhysicalBinaryOp::Eq => Arc::new(kernels::cmp::eq(&lhs, &rhs)?),
                    PhysicalBinaryOp::Add => kernels::numeric::add(&lhs, &rhs)?,
                    PhysicalBinaryOp::Sub => kernels::numeric::sub(&lhs, &rhs)?,
                    PhysicalBinaryOp::Mul => kernels::numeric::mul(&lhs, &rhs)?,
                    PhysicalBinaryOp::Div => kernels::numeric::div(&lhs, &rhs)?,
                }
            }
        };

        Ok(array)
    }
}

impl Display for PhysicalBinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhysicalBinaryOp::Eq => write!(f, "="),
            PhysicalBinaryOp::Add => write!(f, "+"),
            PhysicalBinaryOp::Sub => write!(f, "-"),
            PhysicalBinaryOp::Mul => write!(f, "*"),
            PhysicalBinaryOp::Div => write!(f, "/"),
        }
    }
}

impl Display for PhysicalExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhysicalExpr::Column(index) => {
                write!(f, "#{}", index)
            }
            PhysicalExpr::Literal(physical_literal_expr) => match physical_literal_expr {
                PhysicalLiteralExpr::String(v) => write!(f, "'{}'", v),
                PhysicalLiteralExpr::Long(v) => write!(f, "{}", v),
                PhysicalLiteralExpr::Double(v) => write!(f, "{}", v),
            },
            PhysicalExpr::Binary(physical_binary_expr) => write!(
                f,
                "{} {} {}",
                physical_binary_expr.lhs, physical_binary_expr.op, physical_binary_expr.rhs
            ),
        }
    }
}
