use crate::detect::models::DocumentType;
use crate::document::models::{
    calculate_word_count, compute_blake3_hash, DocumentMetadata, UnifiedDocument,
};
use crate::document::parser::{ContentParser, ParseInput};
use crate::error::CrawlerError;
use crate::models::PageLink;
use async_trait::async_trait;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::{Map, Value};
use url::Url;

pub struct XmlDocumentParser;

impl XmlDocumentParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for XmlDocumentParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContentParser for XmlDocumentParser {
    fn supports(&self, doc_type: DocumentType) -> bool {
        matches!(doc_type, DocumentType::Xml)
    }

    async fn parse(&self, input: ParseInput<'_>) -> Result<UnifiedDocument, CrawlerError> {
        let max_bytes = input.config.max_xml_size_mb * 1024 * 1024;
        if input.bytes.len() > max_bytes {
            return Err(CrawlerError::DocumentTooLarge(input.bytes.len()));
        }

        let max_depth = input.config.max_json_depth;
        let mut reader = Reader::from_reader(input.bytes);
        reader.config_mut().trim_text(true);
        reader.config_mut().expand_empty_elements = true;

        let mut buf = Vec::new();
        let mut depth = 0;
        let mut text_parts = Vec::new();
        let mut links = Vec::new();
        let mut markdown = String::from("# XML Document\n\n");

        let mut root_json = Map::new();
        let mut current_tag = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    depth += 1;
                    if depth > max_depth {
                        return Err(CrawlerError::XmlTooDeep(depth));
                    }
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    current_tag = tag_name.clone();

                    let indent = "  ".repeat(depth.saturating_sub(1));
                    markdown.push_str(&format!("{indent}- **{tag_name}**\n"));

                    // Extract attributes
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let val = String::from_utf8_lossy(&attr.value).to_string();
                        if key.eq_ignore_ascii_case("href") || key.eq_ignore_ascii_case("src") {
                            if let Ok(parsed) = input.final_url.join(&val) {
                                let is_ext = parsed.host_str() != input.final_url.host_str();
                                links.push(PageLink {
                                    url: parsed.to_string(),
                                    text: val.clone(),
                                    rel: Vec::new(),
                                    is_external: is_ext,
                                });
                            }
                        }
                    }
                }
                Ok(Event::Text(e)) => {
                    let text = match e.unescape() {
                        Ok(t) => t.into_owned(),
                        Err(_) => String::from_utf8_lossy(e.as_ref()).to_string(),
                    };
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        text_parts.push(trimmed.to_string());
                        let indent = "  ".repeat(depth);
                        markdown.push_str(&format!("{indent}- {trimmed}\n"));

                        if !current_tag.is_empty() {
                            root_json
                                .insert(current_tag.clone(), Value::String(trimmed.to_string()));
                        }

                        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                            if let Ok(parsed) = Url::parse(trimmed) {
                                let is_ext = parsed.host_str() != input.final_url.host_str();
                                links.push(PageLink {
                                    url: parsed.to_string(),
                                    text: trimmed.to_string(),
                                    rel: Vec::new(),
                                    is_external: is_ext,
                                });
                            }
                        }
                    }
                }
                Ok(Event::CData(e)) => {
                    let text = String::from_utf8_lossy(e.as_ref()).to_string();
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        text_parts.push(trimmed.to_string());
                        let indent = "  ".repeat(depth);
                        markdown.push_str(&format!("{indent}- {trimmed}\n"));
                    }
                }
                Ok(Event::End(_)) => {
                    depth = depth.saturating_sub(1);
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(CrawlerError::XmlParseFailed(format!(
                        "XML parsing error: {e}"
                    )));
                }
                _ => {}
            }
            buf.clear();
        }

        let combined_text = text_parts.join(" ");
        let content_hash = Some(compute_blake3_hash(input.bytes));
        let word_count = Some(calculate_word_count(&combined_text));

        let doc_metadata = DocumentMetadata {
            title: Some("XML Document".to_string()),
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

        Ok(UnifiedDocument {
            source_url: input.url.clone(),
            final_url: input.final_url.clone(),
            document_type: DocumentType::Xml,
            title: Some("XML Document".to_string()),
            text: Some(combined_text),
            markdown: Some(markdown),
            html: None,
            clean_html: None,
            structured_data: Some(Value::Object(root_json)),
            metadata: doc_metadata,
            links,
            images: Vec::new(),
            tables: Vec::new(),
            pages: Vec::new(),
            warnings: Vec::new(),
        })
    }
}
