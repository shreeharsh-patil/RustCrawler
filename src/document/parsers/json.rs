use crate::detect::models::DocumentType;
use crate::document::models::{
    calculate_word_count, compute_blake3_hash, DocumentMetadata, UnifiedDocument,
};
use crate::document::parser::{ContentParser, ParseInput};
use crate::error::CrawlerError;
use crate::models::{PageLink, ScrapeWarning, WarningCode};
use async_trait::async_trait;
use serde_json::Value;
use serde_json_path::JsonPath;
use url::Url;

pub struct JsonDocumentParser;

impl JsonDocumentParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonDocumentParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContentParser for JsonDocumentParser {
    fn supports(&self, doc_type: DocumentType) -> bool {
        matches!(doc_type, DocumentType::Json)
    }

    async fn parse(&self, input: ParseInput<'_>) -> Result<UnifiedDocument, CrawlerError> {
        let max_bytes = input.config.max_json_size_mb * 1024 * 1024;
        if input.bytes.len() > max_bytes {
            return Err(CrawlerError::DocumentTooLarge(input.bytes.len()));
        }

        let mut warnings = Vec::new();

        // Parse root JSON
        let mut json_value: Value = serde_json::from_slice(input.bytes)
            .map_err(|e| CrawlerError::JsonParseFailed(format!("Failed to parse JSON: {e}")))?;

        // Apply JSONPath query if provided in options
        if let Some(ref path_query) = input.options.json_path {
            let path = JsonPath::parse(path_query).map_err(|e| {
                CrawlerError::JsonParseFailed(format!("Invalid JSONPath '{path_query}': {e}"))
            })?;

            let matched_nodes = path.query(&json_value);
            let matched_values: Vec<Value> = matched_nodes.all().into_iter().cloned().collect();

            json_value = if matched_values.len() == 1 {
                matched_values.into_iter().next().unwrap()
            } else {
                Value::Array(matched_values)
            };
        }

        // Validate max nesting depth
        let max_depth = input.config.max_json_depth;
        let actual_depth = calculate_json_depth(&json_value, 1);
        if actual_depth > max_depth {
            return Err(CrawlerError::JsonTooDeep(actual_depth));
        }

        // Render recursive Markdown representation
        let mut markdown_buf = String::from("# JSON Document\n\n");
        let max_array_items = input.config.max_json_array_items;
        render_json_to_markdown(
            &json_value,
            &mut markdown_buf,
            0,
            max_array_items,
            &mut warnings,
        );

        let content_hash = Some(compute_blake3_hash(input.bytes));
        let word_count = Some(calculate_word_count(&markdown_buf));

        // Discover URLs inside JSON string values
        let mut links = Vec::new();
        extract_json_urls(&json_value, input.final_url, &mut links);

        let doc_metadata = DocumentMetadata {
            title: Some("JSON Document".to_string()),
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
            row_count: None,
            word_count,
            open_graph: None,
            twitter: None,
            json_ld: Vec::new(),
            custom: Default::default(),
        };

        let raw_json_str = serde_json::to_string_pretty(&json_value).unwrap_or_default();

        Ok(UnifiedDocument {
            source_url: input.url.clone(),
            final_url: input.final_url.clone(),
            document_type: DocumentType::Json,
            title: Some("JSON Document".to_string()),
            text: Some(raw_json_str.clone()),
            markdown: Some(markdown_buf),
            html: None,
            clean_html: None,
            structured_data: Some(json_value),
            metadata: doc_metadata,
            links,
            images: Vec::new(),
            tables: Vec::new(),
            pages: Vec::new(),
            warnings,
        })
    }
}

fn calculate_json_depth(v: &Value, current_depth: usize) -> usize {
    match v {
        Value::Object(map) => {
            let mut max = current_depth;
            for child in map.values() {
                max = max.max(calculate_json_depth(child, current_depth + 1));
            }
            max
        }
        Value::Array(arr) => {
            let mut max = current_depth;
            for child in arr {
                max = max.max(calculate_json_depth(child, current_depth + 1));
            }
            max
        }
        _ => current_depth,
    }
}

fn render_json_to_markdown(
    v: &Value,
    buf: &mut String,
    indent: usize,
    max_array_items: usize,
    warnings: &mut Vec<ScrapeWarning>,
) {
    let pad = "  ".repeat(indent);
    match v {
        Value::Object(map) => {
            for (key, val) in map {
                match val {
                    Value::Object(_) => {
                        buf.push_str(&format!("{pad}- **{key}:**\n"));
                        render_json_to_markdown(val, buf, indent + 1, max_array_items, warnings);
                    }
                    Value::Array(_) => {
                        buf.push_str(&format!("{pad}## {key}\n"));
                        render_json_to_markdown(val, buf, indent, max_array_items, warnings);
                    }
                    _ => {
                        let scalar_str = scalar_to_string(val);
                        buf.push_str(&format!("{pad}- **{key}:** {scalar_str}\n"));
                    }
                }
            }
        }
        Value::Array(arr) => {
            let count = arr.len();
            let limit = count.min(max_array_items);
            if count > max_array_items {
                warnings.push(ScrapeWarning::new(
                    WarningCode::TruncatedDocument,
                    format!("JSON array truncated at {max_array_items} of {count} items"),
                ));
            }
            for item in &arr[..limit] {
                match item {
                    Value::Object(_) => {
                        buf.push_str(&format!("{pad}-\n"));
                        render_json_to_markdown(item, buf, indent + 1, max_array_items, warnings);
                    }
                    Value::Array(_) => {
                        render_json_to_markdown(item, buf, indent + 1, max_array_items, warnings);
                    }
                    _ => {
                        let scalar_str = scalar_to_string(item);
                        buf.push_str(&format!("{pad}- {scalar_str}\n"));
                    }
                }
            }
        }
        _ => {
            let scalar_str = scalar_to_string(v);
            buf.push_str(&format!("{pad}{scalar_str}\n"));
        }
    }
}

fn scalar_to_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

fn extract_json_urls(v: &Value, base_url: &Url, links: &mut Vec<PageLink>) {
    match v {
        Value::String(s) => {
            if s.starts_with("http://") || s.starts_with("https://") {
                if let Ok(parsed) = Url::parse(s) {
                    let is_ext = parsed.host_str() != base_url.host_str();
                    links.push(PageLink {
                        url: parsed.to_string(),
                        text: s.clone(),
                        rel: Vec::new(),
                        is_external: is_ext,
                    });
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                extract_json_urls(item, base_url, links);
            }
        }
        Value::Object(map) => {
            for val in map.values() {
                extract_json_urls(val, base_url, links);
            }
        }
        _ => {}
    }
}
