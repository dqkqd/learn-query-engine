use std::{fmt::Display, sync::Arc};

use anyhow::Result;
use arrow::{
    array::{ArrayRef, Float64Array, Int64Array, RecordBatch, StringArray},
    compute::kernels,
};
use arrow_schema::ArrowError;


pub enum PhysicalExpr {
    Column(usize),
    Literal(PhysicalLiteralExpr),
    Binary(PhysicalBinaryExpr),
}

pub enum PhysicalLiteralExpr {
    String(String),
    Long(i64),
    Double(f64),
}

pub struct PhysicalBinaryExpr {
    pub lhs: Arc<PhysicalExpr>,
    pub op: PhysicalBinaryOp,
    pub rhs: Arc<PhysicalExpr>,
}

pub enum PhysicalBinaryOp {
    Eq,
    // Neq,
    // Gt,
    // GtEq,
    // Lt,
    // LtEq,
    //
    // Plus,
    // Minus,
    // Multiply,
    // Divide,
    // And,
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
                let result = match physical_binary_expr.op {
                    PhysicalBinaryOp::Eq => kernels::cmp::eq(&lhs, &rhs)?,
                };
                Arc::new(result)
            }
        };

        Ok(array)
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
            PhysicalExpr::Binary(physical_binary_expr) => match physical_binary_expr.op {
                PhysicalBinaryOp::Eq => write!(
                    f,
                    "{} = {}",
                    physical_binary_expr.lhs, physical_binary_expr.rhs
                ),
            },
        }
    }
}
