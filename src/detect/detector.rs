use crate::detect::models::{DetectedContentType, DocumentType};
use url::Url;

/// Multi-signal content-type detection engine.
pub struct ContentDetector;

impl ContentDetector {
    /// Detects the document type from URL, HTTP Content-Type header, and initial body bytes.
    pub fn detect(
        url: &Url,
        content_type_header: Option<&str>,
        body: &[u8],
    ) -> DetectedContentType {
        let (header_mime, header_charset) = parse_content_type_header(content_type_header);
        let extension = get_url_extension(url);

        // 1. Magic byte signatures (Highest precedence for binary formats)
        if is_pdf_magic(body) {
            return DetectedContentType {
                document_type: DocumentType::Pdf,
                mime_type: "application/pdf".to_string(),
                charset: None,
                confidence: 1.0,
                detection_reason: "magic_bytes_pdf".to_string(),
            };
        }

        if is_docx_magic(body, extension.as_deref()) {
            return DetectedContentType {
                document_type: DocumentType::Docx,
                mime_type:
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                        .to_string(),
                charset: None,
                confidence: 0.95,
                detection_reason: "magic_bytes_docx".to_string(),
            };
        }

        // Check BOM for charset
        let (detected_charset, body_after_bom) = detect_bom(body);
        let charset = header_charset.or(detected_charset);

        // 2. Body content inspection on text / structured formats
        let prefix_len = body_after_bom.len().min(8192);
        let prefix_bytes = &body_after_bom[..prefix_len];
        let prefix_str = String::from_utf8_lossy(prefix_bytes);
        let trimmed = prefix_str.trim();

        // 2a. JSON Detection
        if is_json_content(trimmed, body_after_bom) {
            return DetectedContentType {
                document_type: DocumentType::Json,
                mime_type: header_mime.unwrap_or_else(|| "application/json".to_string()),
                charset,
                confidence: 0.95,
                detection_reason: "body_json_structure".to_string(),
            };
        }

        // 2b. Feed Detection (RSS & Atom)
        if is_rss_content(trimmed) {
            return DetectedContentType {
                document_type: DocumentType::Rss,
                mime_type: header_mime.unwrap_or_else(|| "application/rss+xml".to_string()),
                charset,
                confidence: 0.95,
                detection_reason: "body_rss_tags".to_string(),
            };
        }

        if is_atom_content(trimmed) {
            return DetectedContentType {
                document_type: DocumentType::Atom,
                mime_type: header_mime.unwrap_or_else(|| "application/atom+xml".to_string()),
                charset,
                confidence: 0.95,
                detection_reason: "body_atom_tags".to_string(),
            };
        }

        // 2c. HTML Detection
        if is_html_content(trimmed) {
            return DetectedContentType {
                document_type: DocumentType::Html,
                mime_type: header_mime.unwrap_or_else(|| "text/html".to_string()),
                charset,
                confidence: 0.95,
                detection_reason: "body_html_tags".to_string(),
            };
        }

        // 2d. Generic XML Detection
        if is_xml_content(trimmed) {
            return DetectedContentType {
                document_type: DocumentType::Xml,
                mime_type: header_mime.unwrap_or_else(|| "application/xml".to_string()),
                charset,
                confidence: 0.90,
                detection_reason: "body_xml_declaration".to_string(),
            };
        }

        // 2e. CSV / TSV Detection
        if let Some(tabular_type) = detect_csv_or_tsv(trimmed, extension.as_deref()) {
            return DetectedContentType {
                document_type: tabular_type,
                mime_type: header_mime
                    .unwrap_or_else(|| tabular_type.default_mime_type().to_string()),
                charset,
                confidence: 0.85,
                detection_reason: "body_delimited_table".to_string(),
            };
        }

        // 3. Fallback to Content-Type header if body heuristics were inconclusive
        if let Some(ref mime) = header_mime {
            let doc_type = match mime.as_str() {
                "application/pdf" => DocumentType::Pdf,
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
                    DocumentType::Docx
                }
                "application/json" | "application/ld+json" | "text/json" => DocumentType::Json,
                "application/xml" | "text/xml" => DocumentType::Xml,
                "application/rss+xml" | "application/rss" => DocumentType::Rss,
                "application/atom+xml" | "application/atom" => DocumentType::Atom,
                "text/csv" | "application/csv" => DocumentType::Csv,
                "text/tab-separated-values" | "text/tsv" => DocumentType::Tsv,
                "text/plain" | "text/markdown" | "text/x-markdown" => DocumentType::PlainText,
                "text/html" | "application/xhtml+xml" => DocumentType::Html,
                _ => {
                    if mime.ends_with("+json") {
                        DocumentType::Json
                    } else if mime.ends_with("+xml") {
                        DocumentType::Xml
                    } else {
                        DocumentType::Unknown
                    }
                }
            };

            if doc_type != DocumentType::Unknown {
                return DetectedContentType {
                    document_type: doc_type,
                    mime_type: mime.clone(),
                    charset,
                    confidence: 0.80,
                    detection_reason: "header_content_type".to_string(),
                };
            }
        }

        // 4. Fallback to URL file extension
        if let Some(ref ext) = extension {
            let doc_type = match ext.as_str() {
                "pdf" => DocumentType::Pdf,
                "docx" => DocumentType::Docx,
                "json" | "jsonld" => DocumentType::Json,
                "xml" => DocumentType::Xml,
                "rss" => DocumentType::Rss,
                "atom" => DocumentType::Atom,
                "csv" => DocumentType::Csv,
                "tsv" => DocumentType::Tsv,
                "txt" | "text" | "md" | "markdown" => DocumentType::PlainText,
                "html" | "htm" | "xhtml" => DocumentType::Html,
                _ => DocumentType::Unknown,
            };

            if doc_type != DocumentType::Unknown {
                return DetectedContentType {
                    document_type: doc_type,
                    mime_type: header_mime
                        .unwrap_or_else(|| doc_type.default_mime_type().to_string()),
                    charset,
                    confidence: 0.70,
                    detection_reason: "url_extension".to_string(),
                };
            }
        }

        // 5. Final fallback: PlainText if printable text, otherwise Unknown
        if is_printable_text(prefix_bytes) {
            DetectedContentType {
                document_type: DocumentType::PlainText,
                mime_type: header_mime.unwrap_or_else(|| "text/plain".to_string()),
                charset,
                confidence: 0.50,
                detection_reason: "text_heuristics_fallback".to_string(),
            }
        } else {
            DetectedContentType {
                document_type: DocumentType::Unknown,
                mime_type: header_mime.unwrap_or_else(|| "application/octet-stream".to_string()),
                charset,
                confidence: 0.30,
                detection_reason: "unknown_binary".to_string(),
            }
        }
    }
}

fn parse_content_type_header(header: Option<&str>) -> (Option<String>, Option<String>) {
    let header = match header {
        Some(h) if !h.trim().is_empty() => h,
        _ => return (None, None),
    };

    let mut parts = header.split(';');
    let mime = parts.next().map(|m| m.trim().to_lowercase());

    let mut charset = None;
    for part in parts {
        let p = part.trim();
        if let Some(cs) = p.strip_prefix("charset=") {
            charset = Some(
                cs.trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_lowercase(),
            );
            break;
        }
    }

    (mime, charset)
}

fn get_url_extension(url: &Url) -> Option<String> {
    let path = url.path();
    let filename = path.rsplit('/').next()?;
    let ext = filename.rsplit('.').next()?;
    if ext == filename || ext.is_empty() || ext.len() > 8 {
        None
    } else {
        Some(ext.to_lowercase())
    }
}

fn is_pdf_magic(body: &[u8]) -> bool {
    let search_window = &body[..body.len().min(1024)];
    search_window.windows(5).any(|w| w == b"%PDF-")
}

fn is_docx_magic(body: &[u8], extension: Option<&str>) -> bool {
    if body.len() < 4 {
        return false;
    }
    // Check ZIP magic bytes PK\x03\x04 or PK\x05\x06
    let is_zip = (body[0] == b'P' && body[1] == b'K' && body[2] == 0x03 && body[3] == 0x04)
        || (body[0] == b'P' && body[1] == b'K' && body[2] == 0x05 && body[3] == 0x06);

    if !is_zip {
        return false;
    }

    if extension == Some("docx") {
        return true;
    }

    // Check if the zip stream contains standard Office Open XML directory paths
    let search_window = &body[..body.len().min(4096)];
    let search_str = String::from_utf8_lossy(search_window);
    search_str.contains("word/") || search_str.contains("[Content_Types].xml")
}

fn detect_bom(body: &[u8]) -> (Option<String>, &[u8]) {
    if body.starts_with(&[0xEF, 0xBB, 0xBF]) {
        (Some("utf-8".to_string()), &body[3..])
    } else if body.starts_with(&[0xFF, 0xFE]) {
        (Some("utf-16le".to_string()), &body[2..])
    } else if body.starts_with(&[0xFE, 0xFF]) {
        (Some("utf-16be".to_string()), &body[2..])
    } else {
        (None, body)
    }
}

fn is_json_content(trimmed: &str, full_body: &[u8]) -> bool {
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return serde_json::from_slice::<serde_json::Value>(full_body).is_ok();
    }
    false
}

fn is_rss_content(trimmed: &str) -> bool {
    let lower = trimmed.to_lowercase();
    (lower.contains("<rss") && lower.contains("<channel"))
        || (lower.contains("<?xml") && lower.contains("<rss"))
}

fn is_atom_content(trimmed: &str) -> bool {
    let lower = trimmed.to_lowercase();
    (lower.contains("<feed") && lower.contains("http://www.w3.org/2005/atom"))
        || (lower.contains("<?xml") && lower.contains("<feed"))
}

fn is_html_content(trimmed: &str) -> bool {
    let lower = trimmed.to_lowercase();
    lower.starts_with("<!doctype html")
        || lower.starts_with("<html")
        || lower.contains("<html")
        || (lower.contains("<head") && lower.contains("<body"))
}

fn is_xml_content(trimmed: &str) -> bool {
    trimmed.starts_with("<?xml") || (trimmed.starts_with('<') && trimmed.ends_with('>'))
}

fn detect_csv_or_tsv(trimmed: &str, extension: Option<&str>) -> Option<DocumentType> {
    if extension == Some("csv") {
        return Some(DocumentType::Csv);
    }
    if extension == Some("tsv") {
        return Some(DocumentType::Tsv);
    }

    let lines: Vec<&str> = trimmed.lines().take(10).collect();
    if lines.len() < 2 {
        return None;
    }

    // Check comma consistency
    let comma_counts: Vec<usize> = lines.iter().map(|l| l.matches(',').count()).collect();
    if comma_counts[0] >= 1 && comma_counts.iter().all(|&c| c == comma_counts[0]) {
        return Some(DocumentType::Csv);
    }

    // Check tab consistency
    let tab_counts: Vec<usize> = lines.iter().map(|l| l.matches('\t').count()).collect();
    if tab_counts[0] >= 1 && tab_counts.iter().all(|&c| c == tab_counts[0]) {
        return Some(DocumentType::Tsv);
    }

    // Check semicolon consistency
    let semi_counts: Vec<usize> = lines.iter().map(|l| l.matches(';').count()).collect();
    if semi_counts[0] >= 1 && semi_counts.iter().all(|&c| c == semi_counts[0]) {
        return Some(DocumentType::Csv);
    }

    None
}

fn is_printable_text(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }
    let non_printable = bytes
        .iter()
        .filter(|&&b| b < 0x09 || (b > 0x0D && b < 0x20 && b != 0x1B) || b == 0x00)
        .count();

    (non_printable as f32 / bytes.len() as f32) < 0.05
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_large_json_document() {
        let mut large_json = String::from("{\"items\": [");
        for i in 0..1000 {
            large_json.push_str(&format!("{{\"id\": {i}, \"name\": \"item_{i}\"}},"));
        }
        large_json.pop(); // remove trailing comma
        large_json.push_str("]}");

        assert!(large_json.len() > 10000, "Should be larger than 8KB prefix window");

        let url = Url::parse("http://example.com/data").unwrap();
        let detected = ContentDetector::detect(&url, None, large_json.as_bytes());

        assert_eq!(detected.document_type, DocumentType::Json);
        assert_eq!(detected.mime_type, "application/json");
    }

    #[test]
    fn test_detect_pdf_magic_bytes() {
        let pdf_bytes = b"%PDF-1.7\n%raw binary content";
        let url = Url::parse("http://example.com/download").unwrap();
        let detected = ContentDetector::detect(&url, None, pdf_bytes);

        assert_eq!(detected.document_type, DocumentType::Pdf);
    }
}
