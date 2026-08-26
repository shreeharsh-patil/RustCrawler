use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream};
use rustcrawl::config::Config;
use rustcrawl::detect::ContentDetector;
use rustcrawl::document::parser::{ContentParser, ParseInput};
use rustcrawl::document::parsers::docx::DocxDocumentParser;
use rustcrawl::document::parsers::pdf::PdfDocumentParser;
use rustcrawl::models::ScrapeOptions;
use std::io::Write;
use url::Url;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

fn create_test_pdf_bytes() -> Vec<u8> {
    use lopdf::Dictionary;
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let mut font_dict = Dictionary::new();
    font_dict.set("Type", "Font");
    font_dict.set("Subtype", "Type1");
    font_dict.set("BaseFont", "Helvetica");
    let font_id = doc.add_object(font_dict);

    let mut font_res_dict = Dictionary::new();
    font_res_dict.set("F1", font_id);
    let mut res_dict = Dictionary::new();
    res_dict.set("Font", font_res_dict);
    let resources_id = doc.add_object(res_dict);

    let content = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 12.into()]),
            Operation::new("Td", vec![100.into(), 700.into()]),
            Operation::new(
                "Tj",
                vec![Object::string_literal("Hello RustCrawler PDF Universal Extractor! Visit https://example.com/pdf-guide for documentation.")],
            ),
            Operation::new("ET", vec![]),
        ],
    };

    let content_stream = Stream::new(Dictionary::new(), content.encode().unwrap());
    let content_id = doc.add_object(content_stream);

    let mut page_dict = Dictionary::new();
    page_dict.set("Type", "Page");
    page_dict.set("Parent", pages_id);
    page_dict.set("Contents", content_id);
    page_dict.set("Resources", resources_id);
    page_dict.set("MediaBox", vec![0.into(), 0.into(), 595.into(), 842.into()]);
    let page_id = doc.add_object(page_dict);

    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", "Pages");
    pages_dict.set("Kids", vec![page_id.into()]);
    pages_dict.set("Count", 1);
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let mut catalog_dict = Dictionary::new();
    catalog_dict.set("Type", "Catalog");
    catalog_dict.set("Pages", pages_id);
    let catalog_id = doc.add_object(catalog_dict);
    doc.trailer.set("Root", catalog_id);

    let mut pdf_buffer = Vec::new();
    doc.save_to(&mut pdf_buffer).unwrap();
    pdf_buffer
}

fn create_test_docx_bytes() -> Vec<u8> {
    let buffer = std::io::Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(buffer);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // [Content_Types].xml
    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
    <Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
        <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
        <Default Extension="xml" ContentType="application/xml"/>
        <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
    </Types>"#).unwrap();

    // _rels/.rels
    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
    <Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
        <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
    </Relationships>"#).unwrap();

    // word/_rels/document.xml.rels
    zip.start_file("word/_rels/document.xml.rels", options)
        .unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
    <Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
        <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/docx-info" TargetMode="External"/>
    </Relationships>"#).unwrap();

    // word/document.xml
    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
    <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
        <w:body>
            <w:p>
                <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
                <w:r><w:t>DOCX Universal Parser Report</w:t></w:r>
            </w:p>
            <w:p>
                <w:r><w:t>This is an automated document test created for RustCrawler.</w:t></w:r>
            </w:p>
            <w:p>
                <w:hyperlink r:id="rId2">
                    <w:r><w:t>Click here for external link</w:t></w:r>
                </w:hyperlink>
            </w:p>
            <w:tbl>
                <w:tr>
                    <w:tc><w:p><w:r><w:t>Feature</w:t></w:r></w:p></w:tc>
                    <w:tc><w:p><w:r><w:t>Status</w:t></w:r></w:p></w:tc>
                </w:tr>
                <w:tr>
                    <w:tc><w:p><w:r><w:t>OpenXML Extraction</w:t></w:r></w:p></w:tc>
                    <w:tc><w:p><w:r><w:t>Working</w:t></w:r></w:p></w:tc>
                </w:tr>
            </w:tbl>
        </w:body>
    </w:document>"#).unwrap();

    let cursor = zip.finish().unwrap();
    cursor.into_inner()
}

#[tokio::test]
async fn test_pdf_document_parsing() {
    let pdf_bytes = create_test_pdf_bytes();
    let url = Url::parse("https://example.com/test.pdf").unwrap();
    let detected = ContentDetector::detect(&url, Some("application/pdf"), &pdf_bytes);

    let parser = PdfDocumentParser::new();
    let config = Config::default();
    let options = ScrapeOptions::default();

    let input = ParseInput {
        url: &url,
        final_url: &url,
        bytes: &pdf_bytes,
        detected: &detected,
        options: &options,
        config: &config,
    };

    let doc = parser.parse(input).await.expect("PDF parse failed");
    assert_eq!(doc.pages.len(), 1);
    let text = doc.text.expect("Missing PDF text");
    assert!(text.contains("Hello RustCrawler PDF Universal Extractor"));
    assert_eq!(doc.links.len(), 1);
    assert_eq!(doc.links[0].url, "https://example.com/pdf-guide");
}

#[tokio::test]
async fn test_docx_document_parsing() {
    let docx_bytes = create_test_docx_bytes();
    let url = Url::parse("https://example.com/test.docx").unwrap();
    let detected = ContentDetector::detect(&url, None, &docx_bytes);

    let parser = DocxDocumentParser::new();
    let config = Config::default();
    let options = ScrapeOptions::default();

    let input = ParseInput {
        url: &url,
        final_url: &url,
        bytes: &docx_bytes,
        detected: &detected,
        options: &options,
        config: &config,
    };

    let doc = parser.parse(input).await.expect("DOCX parse failed");
    let md = doc.markdown.expect("Missing DOCX markdown");
    assert!(md.contains("# DOCX Universal Parser Report"));
    assert_eq!(doc.links.len(), 1);
    assert_eq!(doc.links[0].url, "https://example.com/docx-info");
    assert_eq!(doc.tables.len(), 1);
    assert_eq!(doc.tables[0].headers, vec!["Feature", "Status"]);
    assert_eq!(doc.tables[0].rows.len(), 1);
    assert_eq!(doc.tables[0].rows[0], vec!["OpenXML Extraction", "Working"]);
}
