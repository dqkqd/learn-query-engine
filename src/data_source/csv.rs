use std::{fs::File, path::PathBuf, sync::Arc};

use anyhow::{Result, bail};
use arrow::{
    array::RecordBatch,
    csv::{ReaderBuilder, reader::Format},
    datatypes::Schema,
    error::ArrowError,
};

use crate::utils::field_ids_by_names;

#[derive(Debug, Clone)]
pub struct CsvDataSource {
    filepath: PathBuf,
}

impl CsvDataSource {
    pub fn new(file: impl AsRef<str>) -> CsvDataSource {
        let filepath = PathBuf::from(file.as_ref());
        CsvDataSource { filepath }
    }

    pub fn schema(&self) -> Result<Arc<Schema>> {
        // TODO: this is a costly operation, consider caching it
        if !self.filepath.exists() {
            bail!("file doesn't exist: {:?}", self.filepath);
        }

        let mut file = File::open(&self.filepath)?;
        let (schema, _) = Format::default()
            .with_header(true)
            .infer_schema(&mut file, Some(100))?;
        Ok(Arc::new(schema))
    }

    pub fn scan(
        self,
        projection: Vec<String>,
    ) -> Result<Box<dyn Iterator<Item = Result<RecordBatch, ArrowError>>>> {
        let schema = self.schema()?;
        let field_ids = field_ids_by_names(&schema, &projection)?;
        let file = File::open(&self.filepath)?;
        let mut builder = ReaderBuilder::new(schema).with_header(true);
        if !field_ids.is_empty() {
            builder = builder.with_projection(field_ids);
        };
        let reader = builder.build(file)?;
        Ok(Box::new(reader))
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use arrow::util::pretty::pretty_format_batches;
    use insta::assert_snapshot;
    use std::io::Write;
    use tempfile::NamedTempFile;

    use crate::data_source::csv::CsvDataSource;

    #[test]
    fn scan_all_columns() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(
            r#"a,b
1,one
2,two
3,three
4,four
5,five
6,six
7,seven
8,eight"#
                .as_bytes(),
        )?;

        let path = file.path();
        let filename = path.to_str().unwrap();
        let csv = CsvDataSource::new(filename);
        let mut scanner = csv.scan(vec!["a".to_string(), "b".to_string()])?;
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
        let mut file = NamedTempFile::new()?;
        file.write_all(
            r#"a,b
1,one
2,two
3,three
4,four
5,five
6,six
7,seven
8,eight"#
                .as_bytes(),
        )?;

        let path = file.path();
        let filename = path.to_str().unwrap();
        let csv = CsvDataSource::new(filename);
        let mut scanner = csv.scan(vec!["a".to_string()])?;
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
        let mut file = NamedTempFile::new()?;
        file.write_all(
            r#"a,b
1,one
2,two
3,three
4,four
5,five
6,six
7,seven
8,eight"#
                .as_bytes(),
        )?;

        let path = file.path();
        let filename = path.to_str().unwrap();
        let csv = CsvDataSource::new(filename);
        let mut scanner = csv.scan(vec![])?;
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
