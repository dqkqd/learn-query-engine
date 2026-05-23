use std::fmt::Display;

use arrow::{array::ArrayRef, compute::kernels};
use arrow_schema::ArrowError;

use crate::physical_plan::expr::PhysicalExpr;

#[derive(Debug)]
pub enum PhysicalAggregateExpr {
    Max(Box<PhysicalExpr>),
    Sum(Box<PhysicalExpr>),
}

#[derive(Debug)]
pub enum Accumulator {
    Max(MaxAccumulator),
    Sum(SumAccumulator),
}

#[derive(Debug)]
pub struct MaxAccumulator {
    value: Option<ArrayRef>,
}

#[derive(Debug)]
pub struct SumAccumulator {
    value: Option<ArrayRef>,
}

impl PhysicalAggregateExpr {
    pub fn input(&self) -> &PhysicalExpr {
        match &self {
            PhysicalAggregateExpr::Max(physical_expr) => physical_expr,
            PhysicalAggregateExpr::Sum(physical_expr) => physical_expr,
        }
    }
    pub fn accumulator(&self) -> Accumulator {
        match &self {
            PhysicalAggregateExpr::Max(_) => Accumulator::Max(MaxAccumulator { value: None }),
            PhysicalAggregateExpr::Sum(_) => Accumulator::Sum(SumAccumulator { value: None }),
        }
    }
}

impl Accumulator {
    pub fn accumulate(&mut self, rhs: ArrayRef) -> Result<(), ArrowError> {
        match self {
            Accumulator::Max(max_accumulator) => max_accumulator.accumulate(rhs),
            Accumulator::Sum(sum_accumulator) => sum_accumulator.accumulate(rhs),
        }
    }

    pub fn value(&self) -> Option<ArrayRef> {
        match self {
            Accumulator::Max(max_accumulator) => max_accumulator.value(),
            Accumulator::Sum(sum_accumulator) => sum_accumulator.value(),
        }
    }

    pub fn merge(&mut self, other: ArrayRef) -> Result<(), ArrowError> {
        match self {
            Accumulator::Max(max_accumulator) => max_accumulator.merge(other),
            Accumulator::Sum(sum_accumulator) => sum_accumulator.merge(other),
        }
    }
}

impl MaxAccumulator {
    fn accumulate(&mut self, rhs: ArrayRef) -> Result<(), ArrowError> {
        match &self.value {
            Some(lhs) => {
                if kernels::cmp::lt(&lhs, &rhs)?.value(0) {
                    self.value = Some(rhs);
                }
            }
            None => self.value = Some(rhs),
        }
        Ok(())
    }

    fn value(&self) -> Option<ArrayRef> {
        self.value.clone()
    }

    fn merge(&mut self, other: ArrayRef) -> Result<(), ArrowError> {
        self.accumulate(other)
    }
}

impl SumAccumulator {
    fn accumulate(&mut self, rhs: ArrayRef) -> Result<(), ArrowError> {
        match &self.value {
            Some(lhs) => self.value = Some(kernels::numeric::add(&lhs, &rhs)?),
            None => self.value = Some(rhs),
        }
        Ok(())
    }

    fn value(&self) -> Option<ArrayRef> {
        self.value.clone()
    }

    fn merge(&mut self, other: ArrayRef) -> Result<(), ArrowError> {
        self.accumulate(other)
    }
}

impl Display for PhysicalAggregateExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhysicalAggregateExpr::Max(physical_expr) => {
                write!(f, "MAX({})", physical_expr)
            }
            PhysicalAggregateExpr::Sum(physical_expr) => {
                write!(f, "SUM({})", physical_expr)
            }
        }
    }
}
