pub mod csv;
pub mod memory;
pub mod parquet;

use std::sync::Arc;

use anyhow::Result;
use arrow::{array::RecordBatch, datatypes::Schema, error::ArrowError};

pub trait DataSource {
    fn schema(&self) -> Arc<Schema>;
    fn scan(
        &self,
        projection: Vec<String>,
    ) -> Result<Box<dyn Iterator<Item = Result<RecordBatch, ArrowError>> + '_>>;
}
