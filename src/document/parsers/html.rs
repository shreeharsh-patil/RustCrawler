use crate::detect::models::DocumentType;
use crate::document::models::{
    calculate_word_count, compute_blake3_hash, DocumentMetadata, UnifiedDocument,
};
use crate::document::parser::{ContentParser, ParseInput};
use crate::error::CrawlerError;
use crate::extract::cleaner::clean_html;
use crate::extract::main_content::extract_main_content;
use crate::extract::markdown::html_to_markdown;
use crate::parser::headings::extract_headings;
use crate::parser::images::extract_images;
use crate::parser::links::extract_links;
use crate::parser::metadata::extract_metadata;
use crate::parser::structured_data::extract_json_ld;
use crate::parser::ParsedDocument as HtmlParsedDoc;
use async_trait::async_trait;

pub struct HtmlDocumentParser;

impl HtmlDocumentParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HtmlDocumentParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContentParser for HtmlDocumentParser {
    fn supports(&self, doc_type: DocumentType) -> bool {
        matches!(doc_type, DocumentType::Html)
    }

    async fn parse(&self, input: ParseInput<'_>) -> Result<UnifiedDocument, CrawlerError> {
        let html_str = match String::from_utf8(input.bytes.to_vec()) {
            Ok(s) => s,
            Err(_) => String::from_utf8_lossy(input.bytes).to_string(),
        };

        let mut warnings = Vec::new();
        let parsed_doc = HtmlParsedDoc::new(&html_str, input.final_url.clone());

        // Extract metadata and structured data
        let mut meta = extract_metadata(&parsed_doc, &mut warnings);
        meta.json_ld = extract_json_ld(&parsed_doc, &mut warnings);
        let links = extract_links(&parsed_doc, &mut warnings);
        let images = extract_images(&parsed_doc);
        let _headings = extract_headings(&parsed_doc);

        // Extract main content, clean HTML, and Markdown
        let main_content_html = extract_main_content(&parsed_doc.html, &mut warnings);
        let intermediate_html = if input.options.only_main_content {
            main_content_html.clone()
        } else {
            html_str.clone()
        };

        let cleaned = clean_html(&intermediate_html);
        let markdown = html_to_markdown(&cleaned, input.final_url);

        let content_hash = Some(compute_blake3_hash(input.bytes));
        let word_count = Some(calculate_word_count(&markdown));

        // Create unified document metadata
        let doc_metadata = DocumentMetadata {
            title: meta.title.clone(),
            description: meta.description,
            author: meta.author,
            subject: None,
            creator: None,
            language: meta.language,
            canonical_url: meta.canonical_url,
            charset: input.detected.charset.clone().or(Some("utf-8".to_string())),
            mime_type: input.detected.mime_type.clone(),
            content_hash,
            published_time: meta.published_time,
            modified_time: meta.modified_time,
            favicon: meta.favicon,
            robots: meta.robots,
            generator: meta.generator,
            page_count: Some(1),
            row_count: None,
            word_count,
            open_graph: meta.open_graph,
            twitter: meta.twitter,
            json_ld: meta.json_ld,
            custom: meta.custom,
        };

        // Text format
        let text_content = if input.options.only_main_content {
            clean_html(&main_content_html)
        } else {
            markdown.clone()
        };

        let structured_json = if !doc_metadata.json_ld.is_empty() {
            Some(serde_json::Value::Array(doc_metadata.json_ld.clone()))
        } else {
            None
        };

        Ok(UnifiedDocument {
            source_url: input.url.clone(),
            final_url: input.final_url.clone(),
            document_type: DocumentType::Html,
            title: doc_metadata.title.clone(),
            text: Some(text_content),
            markdown: Some(markdown),
            html: Some(html_str),
            clean_html: Some(cleaned),
            structured_data: structured_json,
            metadata: doc_metadata,
            links,
            images,
            tables: Vec::new(),
            pages: Vec::new(),
            warnings,
        })
    }
}
