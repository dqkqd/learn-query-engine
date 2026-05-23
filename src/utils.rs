use anyhow::Result;
use arrow::datatypes::Schema;

pub fn field_ids_by_names(schema: &Schema, projection: &[String]) -> Result<Vec<usize>> {
    let mut field_ids = Vec::with_capacity(projection.len());
    for name in projection {
        let field_id = schema.index_of(name)?;
        field_ids.push(field_id);
    }
    Ok(field_ids)
}
