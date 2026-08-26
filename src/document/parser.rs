use crate::config::Config;
use crate::detect::models::{DetectedContentType, DocumentType};
use crate::document::models::UnifiedDocument;
use crate::document::parsers::{
    CsvDocumentParser, DocxDocumentParser, FeedDocumentParser, HtmlDocumentParser,
    JsonDocumentParser, PdfDocumentParser, TextDocumentParser, XmlDocumentParser,
};
use crate::error::CrawlerError;
use crate::models::ScrapeOptions;
use async_trait::async_trait;
use std::sync::Arc;
use url::Url;

/// Input payload provided to a ContentParser.
pub struct ParseInput<'a> {
    pub url: &'a Url,
    pub final_url: &'a Url,
    pub bytes: &'a [u8],
    pub detected: &'a DetectedContentType,
    pub options: &'a ScrapeOptions,
    pub config: &'a Config,
}

/// Generic trait implemented by modular document and content parsers.
#[async_trait]
pub trait ContentParser: Send + Sync {
    /// Returns true if this parser can handle the given DocumentType.
    fn supports(&self, doc_type: DocumentType) -> bool;

    /// Parses input bytes into a normalized UnifiedDocument.
    async fn parse(&self, input: ParseInput<'_>) -> Result<UnifiedDocument, CrawlerError>;
}

/// Registry managing all available content parsers.
#[derive(Clone)]
pub struct ParserRegistry {
    parsers: Vec<Arc<dyn ContentParser>>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        Self {
            parsers: Vec::new(),
        }
    }

    /// Creates a registry pre-loaded with all built-in document parsers.
    pub fn new_default() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(HtmlDocumentParser::new()));
        registry.register(Arc::new(JsonDocumentParser::new()));
        registry.register(Arc::new(XmlDocumentParser::new()));
        registry.register(Arc::new(FeedDocumentParser::new()));
        registry.register(Arc::new(CsvDocumentParser::new()));
        registry.register(Arc::new(TextDocumentParser::new()));
        registry.register(Arc::new(PdfDocumentParser::new()));
        registry.register(Arc::new(DocxDocumentParser::new()));
        registry
    }

    pub fn register(&mut self, parser: Arc<dyn ContentParser>) {
        self.parsers.push(parser);
    }

    /// Finds the first registered parser that supports the detected DocumentType.
    pub fn find_parser(
        &self,
        doc_type: DocumentType,
    ) -> Result<Arc<dyn ContentParser>, CrawlerError> {
        for parser in &self.parsers {
            if parser.supports(doc_type) {
                return Ok(parser.clone());
            }
        }
        Err(CrawlerError::UnsupportedDocumentType(format!(
            "No parser registered for document type '{}'",
            doc_type.as_str()
        )))
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new_default()
    }
}
