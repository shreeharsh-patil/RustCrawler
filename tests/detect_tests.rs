use rustcrawl::detect::models::DocumentType;
use rustcrawl::detect::ContentDetector;
use url::Url;

#[test]
fn test_detect_pdf_magic_bytes() {
    let url = Url::parse("https://example.com/download").unwrap();
    let pdf_bytes = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n1 0 obj\n<<>>\nendobj";
    let detected = ContentDetector::detect(&url, None, pdf_bytes);
    assert_eq!(detected.document_type, DocumentType::Pdf);
    assert_eq!(detected.mime_type, "application/pdf");
    assert!(detected.confidence > 0.9);
}

#[test]
fn test_detect_docx_magic_bytes_and_ext() {
    let url = Url::parse("https://example.com/report.docx").unwrap();
    let docx_bytes = b"PK\x03\x04\x14\x00\x06\x00word/document.xml";
    let detected = ContentDetector::detect(&url, None, docx_bytes);
    assert_eq!(detected.document_type, DocumentType::Docx);
    assert_eq!(
        detected.mime_type,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    );
    assert!(detected.confidence > 0.9);
}

#[test]
fn test_detect_json_from_buffer_and_header() {
    let url = Url::parse("https://api.example.com/v1/data").unwrap();
    let json_bytes = br#"{"status": "ok", "items": [1, 2, 3]}"#;
    let detected =
        ContentDetector::detect(&url, Some("application/json; charset=utf-8"), json_bytes);
    assert_eq!(detected.document_type, DocumentType::Json);
    assert_eq!(detected.mime_type, "application/json");
}

#[test]
fn test_detect_rss_and_atom_feeds() {
    let url = Url::parse("https://example.com/feed").unwrap();
    let rss_bytes = b"<?xml version=\"1.0\"?>\n<rss version=\"2.0\"><channel><title>News</title></channel></rss>";
    let detected_rss = ContentDetector::detect(&url, Some("application/xml"), rss_bytes);
    assert_eq!(detected_rss.document_type, DocumentType::Rss);

    let atom_bytes = b"<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<feed xmlns=\"http://www.w3.org/2005/Atom\"><title>Feed</title></feed>";
    let detected_atom = ContentDetector::detect(&url, None, atom_bytes);
    assert_eq!(detected_atom.document_type, DocumentType::Atom);
}

#[test]
fn test_detect_csv_and_tsv() {
    let url_csv = Url::parse("https://example.com/data.csv").unwrap();
    let csv_bytes = b"id,name,role,email\n1,Alice,Admin,alice@example.com\n2,Bob,User,bob@example.com\n3,Charlie,User,charlie@example.com\n";
    let detected_csv = ContentDetector::detect(&url_csv, None, csv_bytes);
    assert_eq!(detected_csv.document_type, DocumentType::Csv);

    let url_tsv = Url::parse("https://example.com/data.tsv").unwrap();
    let tsv_bytes = b"id\tname\trole\n1\tAlice\tAdmin\n2\tBob\tUser\n";
    let detected_tsv = ContentDetector::detect(&url_tsv, None, tsv_bytes);
    assert_eq!(detected_tsv.document_type, DocumentType::Tsv);
}

#[test]
fn test_detect_html_and_plain_text() {
    let url_html = Url::parse("https://example.com/article").unwrap();
    let html_bytes =
        b"<!DOCTYPE html><html><head><title>Test</title></head><body><h1>Hello</h1></body></html>";
    let detected_html = ContentDetector::detect(&url_html, Some("text/html"), html_bytes);
    assert_eq!(detected_html.document_type, DocumentType::Html);

    let url_txt = Url::parse("https://example.com/notes.txt").unwrap();
    let text_bytes =
        b"This is just a simple plain text notes file with some thoughts.\nLine 2.\nLine 3.";
    let detected_txt = ContentDetector::detect(&url_txt, Some("text/plain"), text_bytes);
    assert_eq!(detected_txt.document_type, DocumentType::PlainText);
}
