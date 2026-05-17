pub mod csv;
pub mod memory;
pub mod parquet;

use std::fmt::Debug;
use std::sync::Arc;

use anyhow::Result;
use arrow::{array::RecordBatch, datatypes::Schema, error::ArrowError};

pub trait DataSource: Debug {
    fn schema(&self) -> Result<Arc<Schema>>;
    fn scan(
        &self,
        projection: Vec<String>,
    ) -> Result<Box<dyn Iterator<Item = Result<RecordBatch, ArrowError>> + '_>>;
}
