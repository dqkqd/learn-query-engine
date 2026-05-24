pub mod csv;
pub mod memory;
pub mod parquet;

use std::fmt::Debug;
use std::sync::Arc;

use anyhow::Result;
use arrow::{array::RecordBatch, datatypes::Schema, error::ArrowError};

use crate::data_source::{
    csv::CsvDataSource, memory::MemoryDataSource, parquet::ParquetDataSource,
};

#[derive(Debug, Clone)]
pub enum DataSource {
    Csv(CsvDataSource),
    Parquet(ParquetDataSource),
    Memory(MemoryDataSource),
}

impl DataSource {
    pub fn schema(&self) -> Result<Arc<Schema>> {
        match self {
            DataSource::Csv(csv_data_source) => csv_data_source.schema(),
            DataSource::Parquet(parquet_data_source) => parquet_data_source.schema(),
            DataSource::Memory(memory_data_source) => memory_data_source.schema(),
        }
    }

    pub fn scan(
        self,
        projection: Vec<String>,
    ) -> Result<Box<dyn Iterator<Item = Result<RecordBatch, ArrowError>>>> {
        match self {
            DataSource::Csv(csv_data_source) => csv_data_source.scan(projection),
            DataSource::Parquet(parquet_data_source) => parquet_data_source.scan(projection),
            DataSource::Memory(memory_data_source) => memory_data_source.scan(projection),
        }
    }
}
