use crate::detect::models::DocumentType;
use crate::document::models::{
    calculate_word_count, compute_blake3_hash, DocumentMetadata, UnifiedDocument,
};
use crate::document::parser::{ContentParser, ParseInput};
use crate::error::CrawlerError;
use crate::models::{PageLink, ScrapeWarning, WarningCode};
use async_trait::async_trait;
use encoding_rs::{Encoding, UTF_16BE, UTF_16LE, UTF_8, WINDOWS_1252};
use url::Url;

pub struct TextDocumentParser;

impl TextDocumentParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TextDocumentParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContentParser for TextDocumentParser {
    fn supports(&self, doc_type: DocumentType) -> bool {
        matches!(doc_type, DocumentType::PlainText)
    }

    async fn parse(&self, input: ParseInput<'_>) -> Result<UnifiedDocument, CrawlerError> {
        let max_bytes = input.config.max_text_size_mb * 1024 * 1024;
        if input.bytes.len() > max_bytes {
            return Err(CrawlerError::DocumentTooLarge(input.bytes.len()));
        }

        let mut warnings = Vec::new();

        // 1. Character decoding with fallback heuristics
        let (decoded_text, used_encoding, had_errors) =
            decode_text_bytes(input.bytes, input.detected.charset.as_deref());

        if had_errors {
            warnings.push(ScrapeWarning::new(
                WarningCode::InvalidEncodingSequence,
                format!("Encountered invalid bytes while decoding with encoding {used_encoding}"),
            ));
        }

        // 2. Normalize newlines
        let normalized = decoded_text.replace("\r\n", "\n").replace('\r', "\n");

        // 3. URLs extraction from plain text
        let mut links = Vec::new();
        for word in normalized.split_whitespace() {
            if word.starts_with("http://") || word.starts_with("https://") {
                let clean_url = word.trim_matches(|c: char| c.is_ascii_punctuation());
                if let Ok(parsed) = Url::parse(clean_url) {
                    let is_ext = parsed.host_str() != input.final_url.host_str();
                    links.push(PageLink {
                        url: parsed.to_string(),
                        text: clean_url.to_string(),
                        rel: Vec::new(),
                        is_external: is_ext,
                    });
                }
            }
        }

        let content_hash = Some(compute_blake3_hash(input.bytes));
        let word_count = Some(calculate_word_count(&normalized));

        let is_markdown = input.detected.mime_type.contains("markdown")
            || input.url.path().ends_with(".md")
            || input.url.path().ends_with(".markdown");

        let doc_metadata = DocumentMetadata {
            title: Some("Plain Text Document".to_string()),
            description: None,
            author: None,
            subject: None,
            creator: None,
            language: None,
            canonical_url: None,
            charset: Some(used_encoding.to_string()),
            mime_type: input.detected.mime_type.clone(),
            content_hash,
            published_time: None,
            modified_time: None,
            favicon: None,
            robots: None,
            generator: None,
            page_count: Some(1),
            row_count: Some(normalized.lines().count()),
            word_count,
            open_graph: None,
            twitter: None,
            json_ld: Vec::new(),
            custom: Default::default(),
        };

        let markdown_content = if is_markdown {
            normalized.clone()
        } else {
            format!("```text\n{}\n```", normalized)
        };

        Ok(UnifiedDocument {
            source_url: input.url.clone(),
            final_url: input.final_url.clone(),
            document_type: DocumentType::PlainText,
            title: Some("Plain Text Document".to_string()),
            text: Some(normalized.clone()),
            markdown: Some(markdown_content),
            html: None,
            clean_html: None,
            structured_data: None,
            metadata: doc_metadata,
            links,
            images: Vec::new(),
            tables: Vec::new(),
            pages: Vec::new(),
            warnings,
        })
    }
}

fn decode_text_bytes(bytes: &[u8], charset_hint: Option<&str>) -> (String, &'static str, bool) {
    if let Some(hint) = charset_hint {
        if let Some(enc) = Encoding::for_label(hint.as_bytes()) {
            let (cow, had_errors) = enc.decode_without_bom_handling(bytes);
            return (cow.into_owned(), enc.name(), had_errors);
        }
    }

    // Check BOM
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let (cow, had_errors) = UTF_8.decode_without_bom_handling(&bytes[3..]);
        return (cow.into_owned(), "utf-8", had_errors);
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        let (cow, had_errors) = UTF_16LE.decode_without_bom_handling(&bytes[2..]);
        return (cow.into_owned(), "utf-16le", had_errors);
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        let (cow, had_errors) = UTF_16BE.decode_without_bom_handling(&bytes[2..]);
        return (cow.into_owned(), "utf-16be", had_errors);
    }

    // Try UTF-8
    match std::str::from_utf8(bytes) {
        Ok(s) => (s.to_string(), "utf-8", false),
        Err(_) => {
            // Fallback to Windows-1252
            let (cow, had_errors) = WINDOWS_1252.decode_without_bom_handling(bytes);
            (cow.into_owned(), "windows-1252", had_errors)
        }
    }
}
