pub mod csv;
pub mod docx;
pub mod feed;
pub mod html;
pub mod json;
pub mod pdf;
pub mod text;
pub mod xml;

pub use self::csv::CsvDocumentParser;
pub use self::docx::DocxDocumentParser;
pub use self::feed::FeedDocumentParser;
pub use self::html::HtmlDocumentParser;
pub use self::json::JsonDocumentParser;
pub use self::pdf::PdfDocumentParser;
pub use self::text::TextDocumentParser;
pub use self::xml::XmlDocumentParser;
