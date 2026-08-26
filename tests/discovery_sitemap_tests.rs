use flate2::write::GzEncoder;
use flate2::Compression;
use rustcrawl::discovery::sitemap::{ParsedSitemapContent, StreamingSitemapParser};
use std::io::Write;

#[test]
fn test_parse_standard_sitemap_xml() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
    <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"
            xmlns:image="http://www.google.com/schemas/sitemap-image/1.1">
        <url>
            <loc>https://example.com/page1</loc>
            <lastmod>2026-08-25</lastmod>
            <changefreq>daily</changefreq>
            <priority>0.8</priority>
            <image:image>
                <image:loc>https://example.com/image1.jpg</image:loc>
            </image:image>
        </url>
        <url>
            <loc>https://example.com/docs/api</loc>
            <lastmod>2026-08-20</lastmod>
            <priority>1.0</priority>
        </url>
    </urlset>"#;

    let result = StreamingSitemapParser::parse_xml_str(xml, 1000).expect("Sitemap should parse");
    match result {
        ParsedSitemapContent::Urls(urls) => {
            assert_eq!(urls.len(), 2);
            assert_eq!(urls[0].loc.as_str(), "https://example.com/page1");
            assert_eq!(urls[0].lastmod.as_deref(), Some("2026-08-25"));
            assert_eq!(urls[0].changefreq.as_deref(), Some("daily"));
            assert_eq!(urls[0].priority, Some(0.8));

            assert_eq!(urls[1].loc.as_str(), "https://example.com/docs/api");
            assert_eq!(urls[1].priority, Some(1.0));
        }
        ParsedSitemapContent::SitemapIndex(_) => panic!("Expected Urls, got SitemapIndex"),
    }
}

#[test]
fn test_parse_sitemap_index_xml() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
    <sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
        <sitemap>
            <loc>https://example.com/sitemap-docs.xml</loc>
            <lastmod>2026-08-25T12:00:00Z</lastmod>
        </sitemap>
        <sitemap>
            <loc>https://example.com/sitemap-blog.xml</loc>
        </sitemap>
    </sitemapindex>"#;

    let result =
        StreamingSitemapParser::parse_xml_str(xml, 1000).expect("Sitemap index should parse");
    match result {
        ParsedSitemapContent::SitemapIndex(sitemaps) => {
            assert_eq!(sitemaps.len(), 2);
            assert_eq!(sitemaps[0].as_str(), "https://example.com/sitemap-docs.xml");
            assert_eq!(sitemaps[1].as_str(), "https://example.com/sitemap-blog.xml");
        }
        ParsedSitemapContent::Urls(_) => panic!("Expected SitemapIndex, got Urls"),
    }
}

#[test]
fn test_parse_gzip_compressed_sitemap() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
    <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
        <url>
            <loc>https://example.com/gzipped-page</loc>
        </url>
    </urlset>"#;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(xml.as_bytes()).unwrap();
    let gzip_bytes = encoder.finish().unwrap();

    let result = StreamingSitemapParser::parse_sitemap_bytes(&gzip_bytes, 1000, 10 * 1024 * 1024)
        .expect("Gzip sitemap should automatically decompress and parse");

    match result {
        ParsedSitemapContent::Urls(urls) => {
            assert_eq!(urls.len(), 1);
            assert_eq!(urls[0].loc.as_str(), "https://example.com/gzipped-page");
        }
        _ => panic!("Expected Urls from gzip sitemap"),
    }
}
