use std::{fs::File, path::PathBuf, sync::Arc};

use anyhow::{Result, bail};
use arrow::{array::RecordBatch, datatypes::Schema, error::ArrowError};
use parquet::arrow::{ProjectionMask, arrow_reader::ParquetRecordBatchReaderBuilder};

use crate::data_source::DataSource;

pub struct ParquetDataSource {
    filepath: PathBuf,
}

impl ParquetDataSource {
    pub fn new(file: impl AsRef<str>) -> ParquetDataSource {
        let filepath = PathBuf::from(file.as_ref());
        ParquetDataSource { filepath }
    }
}

impl DataSource for ParquetDataSource {
    fn schema(&self) -> Result<Arc<Schema>> {
        if !self.filepath.exists() {
            bail!("file doesn't exist: {:?}", self.filepath);
        }
        let file = File::open(&self.filepath)?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
        let schema = builder.schema();
        Ok(Arc::clone(schema))
    }

    fn scan(
        &self,
        projection: Vec<String>,
    ) -> Result<Box<dyn Iterator<Item = Result<RecordBatch, ArrowError>>>> {
        let file = File::open(&self.filepath)?;
        let mut builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
        if !projection.is_empty() {
            let mask = ProjectionMask::columns(
                builder.parquet_schema(),
                projection.iter().map(|v| v.as_str()),
            );
            builder = builder.with_projection(mask);
        }
        let reader = builder.build()?;
        Ok(Box::new(reader))
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use arrow::util::pretty::pretty_format_batches;
    use insta::assert_snapshot;
    use parquet::arrow::ArrowWriter;
    use std::io::Write;
    use tempfile::NamedTempFile;

    use crate::data_source::{DataSource, csv::CsvDataSource, parquet::ParquetDataSource};

    fn csv_to_parquet_file(data: &str) -> Result<NamedTempFile> {
        let mut file = NamedTempFile::new()?;
        file.write_all(data.as_bytes())?;
        let csv = CsvDataSource::new(file.path().to_str().unwrap());
        let reader = csv.scan(vec![])?;

        let parquet_file = NamedTempFile::new()?;
        let mut writer = ArrowWriter::try_new(&parquet_file, csv.schema()?, None).unwrap();

        for batch in reader {
            writer.write(&batch.unwrap()).unwrap();
        }

        writer.close().unwrap();

        Ok(parquet_file)
    }

    #[test]
    fn scan_all_columns() -> Result<()> {
        let parquet_file = csv_to_parquet_file(
            r#"a,b
1,one
2,two
3,three
4,four
5,five
6,six
7,seven
8,eight"#,
        )?;
        let parquet = ParquetDataSource::new(parquet_file.path().to_str().unwrap());
        let mut scanner = parquet.scan(vec!["a".to_string(), "b".to_string()])?;
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
        let parquet_file = csv_to_parquet_file(
            r#"column1,column2
1,one
2,two
3,three
4,four
5,five
6,six
7,seven
8,eight"#,
        )?;
        let parquet = ParquetDataSource::new(parquet_file.path().to_str().unwrap());
        let mut scanner = parquet.scan(vec!["a".to_string()])?;
        let batch = scanner.next().unwrap()?;
        assert_snapshot!(pretty_format_batches(&[batch])?, @"
        ++
        ++
        ++
        ");
        Ok(())
    }

    #[test]
    fn scan_no_column() -> Result<()> {
        let parquet_file = csv_to_parquet_file(
            r#"column1,column2
1,one
2,two
3,three
4,four
5,five
6,six
7,seven
8,eight"#,
        )?;
        let parquet = ParquetDataSource::new(parquet_file.path().to_str().unwrap());
        let mut scanner = parquet.scan(vec![])?;
        let batch = scanner.next().unwrap()?;
        assert_snapshot!(pretty_format_batches(&[batch])?, @"
        +---------+---------+
        | column1 | column2 |
        +---------+---------+
        | 1       | one     |
        | 2       | two     |
        | 3       | three   |
        | 4       | four    |
        | 5       | five    |
        | 6       | six     |
        | 7       | seven   |
        | 8       | eight   |
        +---------+---------+
        ");
        Ok(())
    }
}
