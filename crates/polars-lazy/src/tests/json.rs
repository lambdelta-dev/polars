use std::num::NonZeroUsize;

use super::*;

fn ndjson_sources(rows: &[&[u8]]) -> ScanSources {
    ScanSources::Buffers(
        rows.iter()
            .map(|row| polars_buffer::Buffer::from(row.to_vec()))
            .collect::<Vec<_>>()
            .into(),
    )
}

#[test]
fn test_ndjson_infer_schema_files() -> PolarsResult<()> {
    // A column that is `null` in the first file but a list in a later file should
    // infer the supertype of all files instead of erroring (issue #24843).
    let lf = LazyJsonLineReader::new_with_sources(ndjson_sources(&[
        br#"{"a":null}"#,
        br#"{"a":[1,2]}"#,
    ]))
    .finish()?;
    let df = lf.collect()?;
    assert_eq!(
        df.schema().get("a"),
        Some(&DataType::List(Box::new(DataType::Int64)))
    );

    // Fields that only occur in some of the files are unioned.
    let lf =
        LazyJsonLineReader::new_with_sources(ndjson_sources(&[br#"{"a":1}"#, br#"{"b":"x"}"#]))
            .finish()?;
    let df = lf.collect()?;
    assert_eq!(df.schema().get("a"), Some(&DataType::Int64));
    assert_eq!(df.schema().get("b"), Some(&DataType::String));

    // The same applies to nested fields (this mirrors the reported issue).
    let lf = LazyJsonLineReader::new_with_sources(ndjson_sources(&[
        br#"{"other":{"subtypes":null,"prompt_id":"p1"}}"#,
        br#"{"other":{"subtypes":["x"],"prompt_id":"p2"}}"#,
    ]))
    .finish()?;
    let df = lf.collect()?;
    let DataType::Struct(fields) = df.schema().get("other").unwrap() else {
        panic!("expected a struct column")
    };
    let subtypes = fields
        .iter()
        .find(|field| field.name().as_str() == "subtypes")
        .unwrap();
    assert_eq!(
        subtypes.dtype(),
        &DataType::List(Box::new(DataType::String))
    );

    // Limiting the number of files used for inference falls back to the schema of
    // the first file only.
    let mut lf =
        LazyJsonLineReader::new_with_sources(ndjson_sources(&[br#"{"a":1}"#, br#"{"a":"x"}"#]))
            .with_infer_schema_files(NonZeroUsize::new(1).unwrap())
            .finish()?;
    assert_eq!(lf.collect_schema()?.get("a"), Some(&DataType::Int64));

    Ok(())
}
