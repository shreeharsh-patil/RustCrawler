use crate::detect::models::DocumentType;
use crate::document::models::{
    calculate_word_count, compute_blake3_hash, DocumentMetadata, DocumentTable, UnifiedDocument,
};
use crate::document::parser::{ContentParser, ParseInput};
use crate::error::CrawlerError;
use crate::models::PageLink;
use async_trait::async_trait;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::io::{Cursor, Read};
use url::Url;
use zip::ZipArchive;

pub struct DocxDocumentParser;

impl DocxDocumentParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DocxDocumentParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContentParser for DocxDocumentParser {
    fn supports(&self, doc_type: DocumentType) -> bool {
        matches!(doc_type, DocumentType::Docx)
    }

    async fn parse(&self, input: ParseInput<'_>) -> Result<UnifiedDocument, CrawlerError> {
        let max_bytes = input.config.max_docx_size_mb * 1024 * 1024;
        if input.bytes.len() > max_bytes {
            return Err(CrawlerError::DocumentTooLarge(input.bytes.len()));
        }

        let bytes_owned = input.bytes.to_vec();
        let max_archive_entries = input.config.max_archive_entries;
        let max_uncompressed_bytes = input.config.max_docx_uncompressed_mb * 1024 * 1024;
        let max_ratio = input.config.max_archive_compression_ratio;
        let final_url = input.final_url.clone();
        let source_url = input.url.clone();
        let mime_type = input.detected.mime_type.clone();

        // Run DOCX decompression and XML parsing in a blocking task
        let doc_result = tokio::task::spawn_blocking(move || {
            let cursor = Cursor::new(&bytes_owned);
            let mut archive = ZipArchive::new(cursor)
                .map_err(|e| CrawlerError::DocxArchiveInvalid(format!("Not a valid ZIP archive: {e}")))?;

            // 1. Anti-ZIP-bomb and path validation
            let entry_count = archive.len();
            if entry_count > max_archive_entries {
                return Err(CrawlerError::ArchiveBombDetected(format!(
                    "Archive entry count ({entry_count}) exceeds limit of {max_archive_entries}"
                )));
            }

            let mut total_uncompressed_size: usize = 0;
            for i in 0..entry_count {
                let file = archive.by_index(i).map_err(|e| {
                    CrawlerError::DocxArchiveInvalid(format!("Corrupted entry #{i}: {e}"))
                })?;

                let name = file.name();
                if name.contains("..") || name.starts_with('/') || name.starts_with('\\') {
                    return Err(CrawlerError::ArchiveBombDetected(format!(
                        "Suspicious path traversal entry in archive: {name}"
                    )));
                }

                total_uncompressed_size = total_uncompressed_size.saturating_add(file.size() as usize);
            }

            if total_uncompressed_size > max_uncompressed_bytes {
                return Err(CrawlerError::ArchiveBombDetected(format!(
                    "Total uncompressed size ({total_uncompressed_size} bytes) exceeds limit of {max_uncompressed_bytes} bytes"
                )));
            }

            let compression_ratio = total_uncompressed_size as f64 / bytes_owned.len().max(1) as f64;
            if compression_ratio > max_ratio {
                return Err(CrawlerError::ArchiveBombDetected(format!(
                    "Suspicious compression ratio ({compression_ratio:.1}) exceeds max limit of {max_ratio}"
                )));
            }

            // 2. Read relationships from word/_rels/document.xml.rels (Hyperlinks)
            let mut rel_links: HashMap<String, String> = HashMap::new();
            if let Ok(mut rels_file) = archive.by_name("word/_rels/document.xml.rels") {
                let mut rels_xml = String::new();
                if rels_file.read_to_string(&mut rels_xml).is_ok() {
                    parse_relationships(&rels_xml, &mut rel_links);
                }
            }

            // 3. Read metadata from docProps/core.xml
            let mut props = DocxCoreProperties::default();

            if let Ok(mut props_file) = archive.by_name("docProps/core.xml") {
                let mut props_xml = String::new();
                if props_file.read_to_string(&mut props_xml).is_ok() {
                    props = parse_core_properties(&props_xml);
                }
            }

            // 4. Read main document from word/document.xml
            let mut doc_file = archive
                .by_name("word/document.xml")
                .map_err(|_| CrawlerError::DocxParseFailed("Missing word/document.xml in DOCX archive".to_string()))?;

            let mut doc_xml = String::new();
            doc_file
                .read_to_string(&mut doc_xml)
                .map_err(|e| CrawlerError::DocxParseFailed(format!("Failed to read word/document.xml: {e}")))?;

            let (markdown, raw_text, tables, extracted_links) = parse_document_xml(&doc_xml, &rel_links, &final_url);

            let content_hash = Some(compute_blake3_hash(&bytes_owned));
            let word_count = Some(calculate_word_count(&raw_text));

            let doc_metadata = DocumentMetadata {
                title: props.title.clone(),
                description: props.subject.clone(),
                author: props.creator.clone(),
                subject: props.subject,
                creator: props.creator,
                language: None,
                canonical_url: None,
                charset: Some("utf-8".to_string()),
                mime_type,
                content_hash,
                published_time: props.created,
                modified_time: props.modified,
                favicon: None,
                robots: None,
                generator: Some("Microsoft Word / OpenXML".to_string()),
                page_count: Some(1),
                row_count: None,
                word_count,
                open_graph: None,
                twitter: None,
                json_ld: Vec::new(),
                custom: HashMap::new(),
            };

            Ok(UnifiedDocument {
                source_url,
                final_url,
                document_type: DocumentType::Docx,
                title: props.title,
                text: Some(raw_text),
                markdown: Some(markdown),
                html: None,
                clean_html: None,
                structured_data: None,
                metadata: doc_metadata,
                links: extracted_links,
                images: Vec::new(),
                tables,
                pages: Vec::new(),
                warnings: Vec::new(),
            })
        })
        .await
        .map_err(|e| CrawlerError::InternalError(format!("DOCX worker panicked: {e}")))?;

        doc_result
    }
}

fn parse_relationships(xml: &str, rels: &mut HashMap<String, String>) {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if e.name().as_ref().ends_with(b"Relationship") {
                    let mut id = None;
                    let mut target = None;
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"Id" {
                            id = Some(String::from_utf8_lossy(&attr.value).to_string());
                        } else if attr.key.as_ref() == b"Target" {
                            target = Some(String::from_utf8_lossy(&attr.value).to_string());
                        }
                    }
                    if let (Some(id), Some(target)) = (id, target) {
                        rels.insert(id, target);
                    }
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }
}

#[derive(Default)]
struct DocxCoreProperties {
    title: Option<String>,
    creator: Option<String>,
    subject: Option<String>,
    created: Option<String>,
    modified: Option<String>,
}

fn parse_core_properties(xml: &str) -> DocxCoreProperties {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut current_tag = String::new();
    let mut current_text = String::new();
    let mut props = DocxCoreProperties::default();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                current_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                current_text.clear();
            }
            Ok(Event::Text(e)) => {
                let text = String::from_utf8_lossy(e.as_ref()).to_string();
                current_text.push_str(&text);
            }
            Ok(Event::End(_)) => {
                let trimmed = current_text.trim().to_string();
                if !trimmed.is_empty() {
                    if current_tag.ends_with("title") {
                        props.title = Some(trimmed);
                    } else if current_tag.ends_with("creator") {
                        props.creator = Some(trimmed);
                    } else if current_tag.ends_with("subject") {
                        props.subject = Some(trimmed);
                    } else if current_tag.ends_with("created") {
                        props.created = Some(trimmed);
                    } else if current_tag.ends_with("modified") {
                        props.modified = Some(trimmed);
                    }
                }
                current_tag.clear();
                current_text.clear();
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    props
}

fn parse_document_xml(
    xml: &str,
    rels: &HashMap<String, String>,
    base_url: &Url,
) -> (String, String, Vec<DocumentTable>, Vec<PageLink>) {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut markdown = String::new();
    let mut raw_text = String::new();
    let mut extracted_links = Vec::new();
    let mut tables = Vec::new();

    let mut in_t = false;
    let mut is_bold = false;
    let mut is_italic = false;
    let mut heading_level = 0;
    let mut p_text = String::new();
    let mut current_hyperlink_url: Option<String> = None;

    // Table state
    let mut in_tbl = false;
    let mut current_tbl_rows: Vec<Vec<String>> = Vec::new();
    let mut current_tr_cells: Vec<String> = Vec::new();
    let mut current_cell_text = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let tag = e.name().as_ref().to_vec();
                if tag.ends_with(b":p") || tag == b"p" {
                    heading_level = 0;
                    p_text.clear();
                } else if tag.ends_with(b":pStyle") || tag == b"pStyle" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref().ends_with(b"val") {
                            let val = String::from_utf8_lossy(&attr.value);
                            if val.starts_with("Heading1") || val == "1" {
                                heading_level = 1;
                            } else if val.starts_with("Heading2") || val == "2" {
                                heading_level = 2;
                            } else if val.starts_with("Heading3") || val == "3" {
                                heading_level = 3;
                            }
                        }
                    }
                } else if tag.ends_with(b":r") || tag == b"r" {
                    is_bold = false;
                    is_italic = false;
                } else if tag.ends_with(b":b") || tag == b"b" {
                    is_bold = true;
                } else if tag.ends_with(b":i") || tag == b"i" {
                    is_italic = true;
                } else if tag.ends_with(b":t") || tag == b"t" {
                    in_t = true;
                } else if tag.ends_with(b":hyperlink") || tag == b"hyperlink" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref().ends_with(b":id") || attr.key.as_ref() == b"id" {
                            let id_str = String::from_utf8_lossy(&attr.value).to_string();
                            if let Some(target) = rels.get(&id_str) {
                                current_hyperlink_url = Some(target.clone());
                            }
                        }
                    }
                } else if tag.ends_with(b":tbl") || tag == b"tbl" {
                    in_tbl = true;
                    current_tbl_rows.clear();
                } else if tag.ends_with(b":tr") || tag == b"tr" {
                    current_tr_cells.clear();
                } else if tag.ends_with(b":tc") || tag == b"tc" {
                    current_cell_text.clear();
                }
            }
            Ok(Event::Text(e)) if in_t => {
                let text = String::from_utf8_lossy(e.as_ref()).to_string();
                if in_tbl {
                    current_cell_text.push_str(&text);
                } else {
                    let mut formatted = text.clone();
                    if is_bold {
                        formatted = format!("**{formatted}**");
                    }
                    if is_italic {
                        formatted = format!("*{formatted}*");
                    }
                    if let Some(ref link_url) = current_hyperlink_url {
                        formatted = format!("[{formatted}]({link_url})");
                        if let Ok(parsed) = base_url.join(link_url) {
                            let is_ext = parsed.host_str() != base_url.host_str();
                            extracted_links.push(PageLink {
                                url: parsed.to_string(),
                                text: text.clone(),
                                rel: Vec::new(),
                                is_external: is_ext,
                            });
                        }
                    }
                    p_text.push_str(&formatted);
                    raw_text.push_str(&text);
                    raw_text.push(' ');
                }
            }
            Ok(Event::End(e)) => {
                let tag = e.name().as_ref().to_vec();
                if tag.ends_with(b":t") || tag == b"t" {
                    in_t = false;
                } else if tag.ends_with(b":hyperlink") || tag == b"hyperlink" {
                    current_hyperlink_url = None;
                } else if tag.ends_with(b":p") || tag == b"p" {
                    let trimmed = p_text.trim();
                    if !trimmed.is_empty() {
                        if heading_level > 0 {
                            let hashes = "#".repeat(heading_level);
                            markdown.push_str(&format!("{hashes} {trimmed}\n\n"));
                        } else {
                            markdown.push_str(&format!("{trimmed}\n\n"));
                        }
                    }
                    p_text.clear();
                } else if tag.ends_with(b":tc") || tag == b"tc" {
                    current_tr_cells.push(current_cell_text.trim().to_string());
                    current_cell_text.clear();
                } else if tag.ends_with(b":tr") || tag == b"tr" {
                    if !current_tr_cells.is_empty() {
                        current_tbl_rows.push(current_tr_cells.clone());
                    }
                    current_tr_cells.clear();
                } else if tag.ends_with(b":tbl") || tag == b"tbl" {
                    in_tbl = false;
                    if !current_tbl_rows.is_empty() {
                        let headers = current_tbl_rows[0].clone();
                        let rows = if current_tbl_rows.len() > 1 {
                            current_tbl_rows[1..].to_vec()
                        } else {
                            Vec::new()
                        };

                        markdown.push_str(&format!("| {} |\n", headers.join(" | ")));
                        let div: Vec<&str> = headers.iter().map(|_| "---").collect();
                        markdown.push_str(&format!("| {} |\n", div.join(" | ")));
                        for row in &rows {
                            markdown.push_str(&format!("| {} |\n", row.join(" | ")));
                        }
                        markdown.push('\n');

                        tables.push(DocumentTable {
                            name: Some(format!("Table {}", tables.len() + 1)),
                            headers,
                            rows,
                        });
                    }
                    current_tbl_rows.clear();
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    (markdown, raw_text, tables, extracted_links)
}
