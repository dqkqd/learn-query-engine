use std::sync::Arc;

use anyhow::Result;
use arrow::{array::RecordBatch, datatypes::Schema, error::ArrowError};

#[derive(Debug, Clone)]
pub struct MemoryDataSource {
    schema: Arc<Schema>,
    records: Vec<RecordBatch>,
}

impl MemoryDataSource {
    pub fn new(schema: Arc<Schema>, records: Vec<RecordBatch>) -> MemoryDataSource {
        MemoryDataSource { schema, records }
    }
}

impl MemoryDataSource {
    pub fn schema(&self) -> Result<Arc<Schema>> {
        Ok(Arc::clone(&self.schema))
    }

    pub fn scan(
        self,
        projection: Vec<String>,
    ) -> Result<Box<dyn Iterator<Item = Result<RecordBatch, ArrowError>>>> {
        let records = self.records.into_iter().map(move |batch| {
            let batch = batch.clone();
            let schema = batch.schema();
            let mut field_ids = Vec::with_capacity(projection.len());
            for name in &projection {
                let field_id = schema.index_of(name)?;
                field_ids.push(field_id);
            }
            if !field_ids.is_empty() {
                batch.project(&field_ids)
            } else {
                Ok(batch)
            }
        });
        Ok(Box::new(records))
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use arrow::{array::record_batch, util::pretty::pretty_format_batches};
    use arrow_schema;
    use insta::assert_snapshot;

    use crate::data_source::memory::MemoryDataSource;

    #[test]
    fn scan_all_columns() -> Result<()> {
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
        let in_memory = MemoryDataSource::new(schema, vec![batch]);
        let mut scanner = in_memory.scan(vec!["a".to_string(), "b".to_string()])?;
        let batch = scanner.next().unwrap()?;
        assert_snapshot!(pretty_format_batches(&[batch])?, @"
        +---+-------+
        | a | b     |
        +---+-------+
        | 1 | one   |
        | 2 | two   |
        | 3 | three |
        | 4 | four  |
        | 5 | five  |
        | 6 | six   |
        | 7 | seven |
        | 8 | eight |
        +---+-------+
        ");
        Ok(())
    }

    #[test]
    fn scan_one_column() -> Result<()> {
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
        let in_memory = MemoryDataSource::new(schema, vec![batch]);
        let mut scanner = in_memory.scan(vec!["a".to_string()])?;
        let batch = scanner.next().unwrap()?;
        assert_snapshot!(pretty_format_batches(&[batch])?, @"
        +---+
        | a |
        +---+
        | 1 |
        | 2 |
        | 3 |
        | 4 |
        | 5 |
        | 6 |
        | 7 |
        | 8 |
        +---+
        ");
        Ok(())
    }

    #[test]
    fn scan_no_column() -> Result<()> {
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
        let in_memory = MemoryDataSource::new(schema, vec![batch]);
        let mut scanner = in_memory.scan(vec![])?;
        let batch = scanner.next().unwrap()?;
        assert_snapshot!(pretty_format_batches(&[batch])?, @"
        +---+-------+
        | a | b     |
        +---+-------+
        | 1 | one   |
        | 2 | two   |
        | 3 | three |
        | 4 | four  |
        | 5 | five  |
        | 6 | six   |
        | 7 | seven |
        | 8 | eight |
        +---+-------+
        ");
        Ok(())
    }
}
