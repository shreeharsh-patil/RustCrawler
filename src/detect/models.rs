use serde::{Deserialize, Serialize};

/// Normalized document format classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    #[default]
    Html,
    Json,
    Xml,
    Rss,
    Atom,
    Csv,
    Tsv,
    PlainText,
    Pdf,
    Docx,
    Unknown,
}

impl DocumentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Json => "json",
            Self::Xml => "xml",
            Self::Rss => "rss",
            Self::Atom => "atom",
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::PlainText => "plain_text",
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Unknown => "unknown",
        }
    }

    pub fn is_feed(&self) -> bool {
        matches!(self, Self::Rss | Self::Atom)
    }

    pub fn is_tabular(&self) -> bool {
        matches!(self, Self::Csv | Self::Tsv)
    }

    pub fn default_mime_type(&self) -> &'static str {
        match self {
            Self::Html => "text/html",
            Self::Json => "application/json",
            Self::Xml => "application/xml",
            Self::Rss => "application/rss+xml",
            Self::Atom => "application/atom+xml",
            Self::Csv => "text/csv",
            Self::Tsv => "text/tab-separated-values",
            Self::PlainText => "text/plain",
            Self::Pdf => "application/pdf",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Unknown => "application/octet-stream",
        }
    }
}

impl std::str::FromStr for DocumentType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace('-', "_").as_str() {
            "html" | "htm" | "xhtml" => Ok(DocumentType::Html),
            "json" | "jsonld" | "json_ld" => Ok(DocumentType::Json),
            "xml" => Ok(DocumentType::Xml),
            "rss" => Ok(DocumentType::Rss),
            "atom" => Ok(DocumentType::Atom),
            "csv" => Ok(DocumentType::Csv),
            "tsv" => Ok(DocumentType::Tsv),
            "plain_text" | "text" | "txt" | "markdown" | "md" => Ok(DocumentType::PlainText),
            "pdf" => Ok(DocumentType::Pdf),
            "docx" | "doc" => Ok(DocumentType::Docx),
            "unknown" => Ok(DocumentType::Unknown),
            _ => Err(format!("Unknown document type: '{s}'")),
        }
    }
}

/// Result of content-type detection with confidence scoring and diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedContentType {
    pub document_type: DocumentType,
    pub mime_type: String,
    pub charset: Option<String>,
    pub confidence: f32,
    pub detection_reason: String,
}

impl Default for DetectedContentType {
    fn default() -> Self {
        Self {
            document_type: DocumentType::Html,
            mime_type: "text/html".to_string(),
            charset: Some("utf-8".to_string()),
            confidence: 0.5,
            detection_reason: "default_fallback".to_string(),
        }
    }
}
