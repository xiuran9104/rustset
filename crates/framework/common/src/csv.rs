//! UTF-8 CSV exchange: BOM for spreadsheet compatibility, RFC-style quoting,
//! bounded imports and reversible spreadsheet-formula escaping.
use axum::{body::Body, response::Response};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::BTreeSet;

pub const MAX_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_ROWS: usize = 10_000;

#[derive(Debug, Serialize, JsonSchema, Default)]
pub struct ImportResult {
    pub created: usize,
    pub failed: usize,
    pub errors: Vec<ImportError>,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct ImportError {
    pub row: usize,
    pub error: String,
}
impl ImportResult {
    pub fn failed(&mut self, row: usize, error: impl Into<String>) {
        self.failed += 1;
        if self.errors.len() < 50 {
            self.errors.push(ImportError {
                row,
                error: error.into(),
            });
        }
    }
}

pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

fn needs_escape(value: &str) -> bool {
    value.starts_with('\'')
        || value.starts_with(['\t', '\r', '\n'])
        || value.trim_start().starts_with(['=', '+', '-', '@'])
}
fn unescape(value: &str) -> String {
    match value.strip_prefix('\'') {
        Some(rest) if needs_escape(rest) => rest.to_owned(),
        _ => value.to_owned(),
    }
}

pub fn encode(headers: &[&str], rows: &[Vec<String>]) -> Result<Vec<u8>, String> {
    let mut writer = ::csv::WriterBuilder::new()
        .terminator(::csv::Terminator::CRLF)
        .from_writer(vec![0xef, 0xbb, 0xbf]);
    for row in std::iter::once(headers.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .chain(rows.iter().cloned())
    {
        writer
            .write_record(row.iter().map(|v| {
                if needs_escape(v) {
                    format!("'{v}")
                } else {
                    v.clone()
                }
            }))
            .map_err(|e| e.to_string())?;
    }
    writer.into_inner().map_err(|e| e.to_string())
}

pub fn decode(bytes: &[u8]) -> Result<Table, String> {
    if bytes.len() > MAX_BYTES {
        return Err("CSV 文件不得超过 5 MiB".into());
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "CSV 必须使用 UTF-8 编码")?
        .trim_start_matches('\u{feff}');
    let mut reader = ::csv::ReaderBuilder::new().from_reader(text.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|v| unescape(v).trim().to_owned())
        .collect();
    if headers.is_empty()
        || headers.iter().any(String::is_empty)
        || headers.iter().collect::<BTreeSet<_>>().len() != headers.len()
    {
        return Err("CSV 表头不能为空或重复".into());
    }
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| format!("CSV 格式错误: {e}"))?;
        if rows.len() >= MAX_ROWS {
            return Err("CSV 最多包含 10000 条记录".into());
        }
        rows.push(record.iter().map(unescape).collect());
    }
    Ok(Table { headers, rows })
}

pub fn response(
    filename: &str,
    headers: &[&str],
    rows: &[Vec<String>],
) -> Result<Response, String> {
    Response::builder()
        .header("content-type", "text/csv; charset=utf-8")
        .header(
            "content-disposition",
            format!("attachment; filename=\"{filename}\""),
        )
        .body(Body::from(encode(headers, rows)?))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_quotes_unicode_newlines_and_formula_text() {
        let row = vec![
            "中文,列",
            "a\"b\nc",
            "=SUM(A1)",
            "'literal",
            "-10",
            "  +1",
            "\t@x",
            "",
        ];
        let rows = vec![row.iter().map(|v| v.to_string()).collect::<Vec<_>>()];
        let data = encode(&["a", "b", "c", "d", "e", "f", "g", "h"], &rows).unwrap();
        assert!(String::from_utf8_lossy(&data).contains("'=SUM"));
        assert_eq!(decode(&data).unwrap().rows, rows);
    }
    #[test]
    fn rejects_ambiguous_headers_invalid_utf8_and_limits() {
        for bad in [b"a,a\n1,2".as_slice(), b"a,b\n1", b"", b"a\n\xff"] {
            assert!(decode(bad).is_err());
        }
        assert!(decode(&vec![b'a'; MAX_BYTES + 1]).is_err());
    }
}
