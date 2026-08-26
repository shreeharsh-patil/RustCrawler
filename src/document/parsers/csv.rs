use crate::detect::models::DocumentType;
use crate::document::models::{
    calculate_word_count, compute_blake3_hash, DocumentMetadata, DocumentTable, UnifiedDocument,
};
use crate::document::parser::{ContentParser, ParseInput};
use crate::error::CrawlerError;
use crate::models::{ScrapeWarning, WarningCode};
use async_trait::async_trait;
use csv::ReaderBuilder;
use serde_json::json;

pub struct CsvDocumentParser;

impl CsvDocumentParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CsvDocumentParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContentParser for CsvDocumentParser {
    fn supports(&self, doc_type: DocumentType) -> bool {
        matches!(doc_type, DocumentType::Csv | DocumentType::Tsv)
    }

    async fn parse(&self, input: ParseInput<'_>) -> Result<UnifiedDocument, CrawlerError> {
        let max_bytes = input.config.max_csv_size_mb * 1024 * 1024;
        if input.bytes.len() > max_bytes {
            return Err(CrawlerError::DocumentTooLarge(input.bytes.len()));
        }

        let delimiter = if input.detected.document_type == DocumentType::Tsv {
            b'\t'
        } else {
            detect_delimiter(input.bytes)
        };

        let mut rdr = ReaderBuilder::new()
            .delimiter(delimiter)
            .has_headers(true)
            .flexible(true)
            .from_reader(input.bytes);

        let mut warnings = Vec::new();
        let max_rows = input.config.max_table_rows;
        let max_cols = input.config.max_table_columns;

        // Extract headers
        let headers: Vec<String> = match rdr.headers() {
            Ok(h) => h
                .iter()
                .take(max_cols)
                .map(|s| s.trim().to_string())
                .collect(),
            Err(e) => {
                return Err(CrawlerError::CsvParseFailed(format!(
                    "Failed to read CSV headers: {e}"
                )))
            }
        };

        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut row_count = 0;

        for result in rdr.records() {
            row_count += 1;
            if row_count > max_rows {
                warnings.push(ScrapeWarning::new(
                    WarningCode::TruncatedTable,
                    format!("CSV rows truncated at {max_rows} limit"),
                ));
                break;
            }

            match result {
                Ok(record) => {
                    let row: Vec<String> = record
                        .iter()
                        .take(max_cols)
                        .map(|s| s.trim().to_string())
                        .collect();
                    rows.push(row);
                }
                Err(e) => {
                    warnings.push(ScrapeWarning::new(
                        WarningCode::MalformedRow,
                        format!("Malformed CSV row #{row_count}: {e}"),
                    ));
                }
            }
        }

        let table = DocumentTable {
            name: Some("Table 1".to_string()),
            headers: headers.clone(),
            rows: rows.clone(),
        };

        // Render Markdown Table
        let mut markdown = format!("# CSV Table ({} rows)\n\n", rows.len());
        if !headers.is_empty() {
            markdown.push_str(&format!("| {} |\n", headers.join(" | ")));
            let dividers: Vec<&str> = headers.iter().map(|_| "---").collect();
            markdown.push_str(&format!("| {} |\n", dividers.join(" | ")));
        }
        for row in rows.iter().take(100) {
            markdown.push_str(&format!("| {} |\n", row.join(" | ")));
        }
        if rows.len() > 100 {
            markdown.push_str(&format!(
                "\n*... ({} additional rows truncated in preview)*\n",
                rows.len() - 100
            ));
        }

        // Build structured JSON
        let mut json_rows = Vec::new();
        for row in &rows {
            let mut obj = serde_json::Map::new();
            for (idx, header) in headers.iter().enumerate() {
                let cell_val = row.get(idx).cloned().unwrap_or_default();
                obj.insert(header.clone(), serde_json::Value::String(cell_val));
            }
            json_rows.push(serde_json::Value::Object(obj));
        }

        let structured_data = json!({
            "headers": headers,
            "row_count": rows.len(),
            "records": json_rows,
        });

        let content_hash = Some(compute_blake3_hash(input.bytes));
        let word_count = Some(calculate_word_count(&markdown));

        let doc_metadata = DocumentMetadata {
            title: Some("CSV Data".to_string()),
            description: None,
            author: None,
            subject: None,
            creator: None,
            language: None,
            canonical_url: None,
            charset: input.detected.charset.clone().or(Some("utf-8".to_string())),
            mime_type: input.detected.mime_type.clone(),
            content_hash,
            published_time: None,
            modified_time: None,
            favicon: None,
            robots: None,
            generator: None,
            page_count: Some(1),
            row_count: Some(rows.len()),
            word_count,
            open_graph: None,
            twitter: None,
            json_ld: Vec::new(),
            custom: Default::default(),
        };

        Ok(UnifiedDocument {
            source_url: input.url.clone(),
            final_url: input.final_url.clone(),
            document_type: input.detected.document_type,
            title: Some("CSV Data".to_string()),
            text: Some(markdown.clone()),
            markdown: Some(markdown),
            html: None,
            clean_html: None,
            structured_data: Some(structured_data),
            metadata: doc_metadata,
            links: Vec::new(),
            images: Vec::new(),
            tables: vec![table],
            pages: Vec::new(),
            warnings,
        })
    }
}

fn detect_delimiter(bytes: &[u8]) -> u8 {
    let prefix = &bytes[..bytes.len().min(4096)];
    let s = String::from_utf8_lossy(prefix);
    let first_line = s.lines().next().unwrap_or("");

    let commas = first_line.matches(',').count();
    let tabs = first_line.matches('\t').count();
    let semis = first_line.matches(';').count();
    let pipes = first_line.matches('|').count();

    if tabs > commas && tabs > semis && tabs > pipes {
        b'\t'
    } else if semis > commas && semis > tabs && semis > pipes {
        b';'
    } else if pipes > commas && pipes > tabs && pipes > semis {
        b'|'
    } else {
        b','
    }
}
