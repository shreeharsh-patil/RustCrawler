use crate::detect::models::DocumentType;
use crate::document::models::{
    calculate_word_count, compute_blake3_hash, DocumentMetadata, DocumentPage, UnifiedDocument,
};
use crate::document::parser::{ContentParser, ParseInput};
use crate::error::CrawlerError;
use crate::models::{PageLink, ScrapeWarning, WarningCode};
use async_trait::async_trait;
use lopdf::{Document, Object};
use url::Url;

pub struct PdfDocumentParser;

impl PdfDocumentParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PdfDocumentParser {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
struct PdfInfo {
    title: Option<String>,
    author: Option<String>,
    subject: Option<String>,
    creator: Option<String>,
    creation_date: Option<String>,
}

#[async_trait]
impl ContentParser for PdfDocumentParser {
    fn supports(&self, doc_type: DocumentType) -> bool {
        doc_type == DocumentType::Pdf
    }

    async fn parse(&self, input: ParseInput<'_>) -> Result<UnifiedDocument, CrawlerError> {
        let bytes = input.bytes.to_vec();
        let max_pages = input.config.max_pdf_pages;
        let max_text_bytes = input.config.max_pdf_extracted_text_bytes();
        let source_url = input.url.clone();
        let final_url = input.final_url.clone();

        // PDF extraction involves CPU-bound parsing; run on blocking thread pool
        let parsed =
            tokio::task::spawn_blocking(move || -> Result<UnifiedDocument, CrawlerError> {
                let doc = Document::load_mem(&bytes).map_err(|e| {
                    CrawlerError::PdfParseFailed(format!("PDF decompression error: {e}"))
                })?;

                if doc.is_encrypted() {
                    return Err(CrawlerError::PdfEncrypted);
                }

                let page_numbers: Vec<u32> = doc.get_pages().keys().copied().collect();
                let page_count = page_numbers.len();

                if page_count > max_pages {
                    return Err(CrawlerError::PdfPageLimitExceeded(page_count));
                }

                // Extract metadata from Info dictionary
                let pdf_info = extract_pdf_info(&doc);

                let mut pages = Vec::new();
                let mut total_text = String::new();
                let mut warnings = Vec::new();

                for (idx, &page_num) in page_numbers.iter().enumerate() {
                    let page_text = doc.extract_text(&[page_num]).unwrap_or_default();
                    let clean_text = page_text.trim().to_string();

                    if total_text.len() + clean_text.len() > max_text_bytes {
                        warnings.push(ScrapeWarning::new(
                            WarningCode::TruncatedDocument,
                            format!(
                                "PDF text extraction exceeded max limit of {} MB",
                                max_text_bytes / (1024 * 1024)
                            ),
                        ));
                        break;
                    }

                    total_text.push_str(&clean_text);
                    total_text.push('\n');

                    pages.push(DocumentPage {
                        page_number: idx + 1,
                        text: clean_text,
                    });
                }

                // Detect scanned / image-only PDF
                let total_trimmed = total_text.trim();
                if total_trimmed.len() < 50 && page_count > 0 {
                    warnings.push(ScrapeWarning::new(
                        WarningCode::ScannedPdfDetected,
                        "PDF appears to contain only scanned raster images; OCR is not supported",
                    ));
                }

                // Extract URLs embedded in extracted text
                let mut links = Vec::new();
                for word in total_trimmed.split_whitespace() {
                    let clean_url = word.trim_matches(|c: char| {
                        !c.is_alphanumeric()
                            && c != '/'
                            && c != ':'
                            && c != '.'
                            && c != '?'
                            && c != '&'
                            && c != '='
                    });
                    if clean_url.starts_with("http://") || clean_url.starts_with("https://") {
                        if let Ok(parsed) = Url::parse(clean_url) {
                            let is_ext = parsed.host_str() != final_url.host_str();
                            links.push(PageLink {
                                url: parsed.to_string(),
                                text: clean_url.to_string(),
                                rel: Vec::new(),
                                is_external: is_ext,
                            });
                        }
                    }
                }

                // Markdown representation
                let mut markdown = String::new();
                if let Some(ref t) = pdf_info.title {
                    markdown.push_str(&format!("# {t}\n\n"));
                }

                for page in &pages {
                    if pages.len() > 1 {
                        markdown.push_str(&format!("## Page {}\n\n", page.page_number));
                    }
                    markdown.push_str(&page.text);
                    markdown.push_str("\n\n");
                }

                let word_count = calculate_word_count(total_trimmed);
                let content_hash = compute_blake3_hash(total_trimmed.as_bytes());

                let metadata = DocumentMetadata {
                    title: pdf_info.title.clone(),
                    description: pdf_info.subject.clone(),
                    author: pdf_info.author,
                    subject: pdf_info.subject,
                    creator: pdf_info.creator,
                    page_count: Some(page_count),
                    word_count: Some(word_count),
                    content_hash: Some(content_hash),
                    published_time: pdf_info.creation_date,
                    ..Default::default()
                };

                Ok(UnifiedDocument {
                    source_url,
                    final_url,
                    title: pdf_info.title,
                    document_type: DocumentType::Pdf,
                    markdown: if total_trimmed.is_empty() {
                        None
                    } else {
                        Some(markdown.trim().to_string())
                    },
                    text: if total_trimmed.is_empty() {
                        None
                    } else {
                        Some(total_trimmed.to_string())
                    },
                    html: None,
                    clean_html: None,
                    structured_data: None,
                    metadata,
                    links,
                    images: Vec::new(),
                    tables: Vec::new(),
                    pages,
                    warnings,
                })
            })
            .await
            .map_err(|join_err| {
                CrawlerError::InternalError(format!("PDF worker panicked: {join_err}"))
            })?;

        parsed
    }
}

fn extract_pdf_info(doc: &Document) -> PdfInfo {
    let mut info = PdfInfo::default();

    if let Ok(info_dict) = doc
        .trailer
        .get(b"Info")
        .and_then(|obj| doc.dereference(obj))
    {
        if let Ok(dict) = info_dict.1.as_dict() {
            if let Ok(t) = dict.get(b"Title").and_then(|v| doc.dereference(v)) {
                info.title = extract_string_object(t.1);
            }
            if let Ok(a) = dict.get(b"Author").and_then(|v| doc.dereference(v)) {
                info.author = extract_string_object(a.1);
            }
            if let Ok(s) = dict.get(b"Subject").and_then(|v| doc.dereference(v)) {
                info.subject = extract_string_object(s.1);
            }
            if let Ok(c) = dict.get(b"Creator").and_then(|v| doc.dereference(v)) {
                info.creator = extract_string_object(c.1);
            }
            if let Ok(d) = dict.get(b"CreationDate").and_then(|v| doc.dereference(v)) {
                info.creation_date = extract_string_object(d.1);
            }
        }
    }

    info
}

#[allow(clippy::chunks_exact_to_as_chunks)]
fn extract_string_object(obj: &Object) -> Option<String> {
    match obj {
        Object::String(bytes, _) => {
            if bytes.starts_with(&[0xFE, 0xFF]) {
                // UTF-16BE
                let u16_vec: Vec<u16> = bytes[2..]
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                String::from_utf16(&u16_vec).ok()
            } else {
                Some(String::from_utf8_lossy(bytes).to_string())
            }
        }
        Object::Name(bytes) => Some(String::from_utf8_lossy(bytes).to_string()),
        _ => None,
    }
}
