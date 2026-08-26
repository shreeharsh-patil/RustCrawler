use crate::fetch::HttpFetcher;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::HashSet;
use tracing::{debug, warn};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SitemapEntry {
    Url(String),
    SitemapIndex(String),
}

/// Parses XML sitemap content and extracts all `<loc>` URLs from `<url>` or `<sitemap>` elements.
pub fn parse_sitemap_xml(xml_content: &str) -> Vec<SitemapEntry> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut entries = Vec::new();
    let mut in_loc = false;
    let mut in_url = false;
    let mut in_sitemap = false;
    let mut current_loc = String::new();

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.local_name();
                if name.as_ref() == b"url" {
                    in_url = true;
                } else if name.as_ref() == b"sitemap" {
                    in_sitemap = true;
                } else if name.as_ref() == b"loc" {
                    in_loc = true;
                    current_loc.clear();
                }
            }
            Ok(Event::Text(e)) => {
                if in_loc {
                    if let Ok(text) = e.unescape() {
                        current_loc.push_str(&text);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.local_name();
                if name.as_ref() == b"loc" {
                    in_loc = false;
                    let trimmed = current_loc.trim();
                    if !trimmed.is_empty() {
                        if in_sitemap {
                            entries.push(SitemapEntry::SitemapIndex(trimmed.to_string()));
                        } else if in_url {
                            entries.push(SitemapEntry::Url(trimmed.to_string()));
                        } else {
                            // Fallback if neither parent was explicitly tracked
                            entries.push(SitemapEntry::Url(trimmed.to_string()));
                        }
                    }
                } else if name.as_ref() == b"url" {
                    in_url = false;
                } else if name.as_ref() == b"sitemap" {
                    in_sitemap = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                warn!("Error parsing sitemap XML: {:?}", err);
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    entries
}

/// Discovers and recursively fetches sitemap URLs, respecting max depth and index limits.
pub async fn discover_sitemap_urls(
    seed_url: &Url,
    custom_sitemaps: &[String],
    fetcher: &HttpFetcher,
    max_urls: usize,
) -> Vec<Url> {
    let mut discovered = Vec::new();
    let mut visited_sitemaps = HashSet::new();
    let mut sitemaps_to_fetch = Vec::new();

    // 1. Add custom/robots.txt declared sitemaps
    for sm in custom_sitemaps {
        if let Ok(u) = Url::parse(sm) {
            sitemaps_to_fetch.push(u);
        }
    }

    // 2. Add default /sitemap.xml if no custom sitemaps provided
    if sitemaps_to_fetch.is_empty() {
        let base_str = format!(
            "{}://{}",
            seed_url.scheme(),
            seed_url.host_str().unwrap_or("")
        );
        if let Ok(default_sm) = Url::parse(&format!("{base_str}/sitemap.xml")) {
            sitemaps_to_fetch.push(default_sm);
        }
    }

    let mut depth = 0;
    const MAX_SITEMAP_DEPTH: usize = 3;

    while !sitemaps_to_fetch.is_empty() && depth < MAX_SITEMAP_DEPTH && discovered.len() < max_urls
    {
        let current_batch = std::mem::take(&mut sitemaps_to_fetch);
        depth += 1;

        for sitemap_url in current_batch {
            let url_str = sitemap_url.to_string();
            if !visited_sitemaps.insert(url_str.clone()) {
                continue;
            }

            debug!("Fetching sitemap at {}", sitemap_url);
            match fetcher.fetch(sitemap_url.as_str()).await {
                Ok(doc) => {
                    if !(200..300).contains(&doc.status) {
                        debug!("Sitemap {} returned status {}", sitemap_url, doc.status);
                        continue;
                    }

                    let entries = parse_sitemap_xml(&doc.html);
                    for entry in entries {
                        match entry {
                            SitemapEntry::Url(loc) => {
                                if let Ok(u) = Url::parse(&loc) {
                                    if u.scheme() == "http" || u.scheme() == "https" {
                                        discovered.push(u);
                                        if discovered.len() >= max_urls {
                                            break;
                                        }
                                    }
                                }
                            }
                            SitemapEntry::SitemapIndex(sub_sitemap) => {
                                if let Ok(u) = Url::parse(&sub_sitemap) {
                                    if !visited_sitemaps.contains(u.as_str()) {
                                        sitemaps_to_fetch.push(u);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    debug!("Failed to fetch sitemap {}: {}", sitemap_url, e);
                }
            }

            if discovered.len() >= max_urls {
                break;
            }
        }
    }

    discovered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_urlset_sitemap() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/page1</loc>
    <lastmod>2026-08-25</lastmod>
    <changefreq>daily</changefreq>
    <priority>0.8</priority>
  </url>
  <url>
    <loc>https://example.com/page2</loc>
  </url>
</urlset>"#;

        let entries = parse_sitemap_xml(xml);
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0],
            SitemapEntry::Url("https://example.com/page1".to_string())
        );
        assert_eq!(
            entries[1],
            SitemapEntry::Url("https://example.com/page2".to_string())
        );
    }

    #[test]
    fn test_parse_sitemap_index() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <sitemap>
    <loc>https://example.com/sub-sitemap1.xml</loc>
    <lastmod>2026-08-25</lastmod>
  </sitemap>
  <sitemap>
    <loc>https://example.com/sub-sitemap2.xml.gz</loc>
  </sitemap>
</sitemapindex>"#;

        let entries = parse_sitemap_xml(xml);
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0],
            SitemapEntry::SitemapIndex("https://example.com/sub-sitemap1.xml".to_string())
        );
        assert_eq!(
            entries[1],
            SitemapEntry::SitemapIndex("https://example.com/sub-sitemap2.xml.gz".to_string())
        );
    }
}
