use crate::error::CrawlerError;
use flate2::read::GzDecoder;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::io::Read;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SitemapEntry {
    pub loc: Url,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lastmod: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changefreq: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedSitemapContent {
    Urls(Vec<SitemapEntry>),
    SitemapIndex(Vec<Url>),
}

pub struct StreamingSitemapParser;

impl StreamingSitemapParser {
    /// Parses sitemap bytes (automatically detecting and decompressing Gzip if needed).
    pub fn parse_sitemap_bytes(
        bytes: &[u8],
        max_urls: usize,
        max_file_size_bytes: usize,
    ) -> Result<ParsedSitemapContent, CrawlerError> {
        if bytes.len() > max_file_size_bytes {
            return Err(CrawlerError::DocumentTooLarge(bytes.len()));
        }

        // Detect Gzip magic number (0x1f, 0x8b)
        let decompressed_xml: String = if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
            let mut decoder = GzDecoder::new(bytes);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed).map_err(|e| {
                CrawlerError::SitemapParseFailed(format!("Gzip decompression failed: {e}"))
            })?;

            if decompressed.len() > max_file_size_bytes {
                return Err(CrawlerError::DocumentTooLarge(decompressed.len()));
            }

            String::from_utf8_lossy(&decompressed).to_string()
        } else {
            String::from_utf8_lossy(bytes).to_string()
        };

        Self::parse_xml_str(&decompressed_xml, max_urls)
    }

    /// Streaming XML parser extracting `<url>` items and `<sitemap>` index entries.
    pub fn parse_xml_str(
        xml_content: &str,
        max_urls: usize,
    ) -> Result<ParsedSitemapContent, CrawlerError> {
        let mut reader = Reader::from_str(xml_content);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();
        let mut is_sitemap_index = false;

        let mut entries = Vec::new();
        let mut index_sitemaps = Vec::new();

        let mut current_tag = String::new();
        let mut in_url_or_sitemap = false;

        let mut current_loc: Option<String> = None;
        let mut current_lastmod: Option<String> = None;
        let mut current_changefreq: Option<String> = None;
        let mut current_priority: Option<String> = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    let local_name = e.local_name();
                    let tag_name = String::from_utf8_lossy(local_name.as_ref()).to_lowercase();

                    if tag_name == "sitemapindex" {
                        is_sitemap_index = true;
                    } else if tag_name == "url" || tag_name == "sitemap" {
                        in_url_or_sitemap = true;
                        current_loc = None;
                        current_lastmod = None;
                        current_changefreq = None;
                        current_priority = None;
                    } else if in_url_or_sitemap {
                        current_tag = tag_name;
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if in_url_or_sitemap {
                        let text = e.unescape().unwrap_or_default().trim().to_string();
                        if !text.is_empty() {
                            match current_tag.as_str() {
                                "loc" => current_loc = Some(text),
                                "lastmod" => current_lastmod = Some(text),
                                "changefreq" => current_changefreq = Some(text),
                                "priority" => current_priority = Some(text),
                                _ => {} // Ignore image:loc, news:loc or other unknown namespace attributes safely
                            }
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    let local_name = e.local_name();
                    let tag_name = String::from_utf8_lossy(local_name.as_ref()).to_lowercase();

                    if tag_name == "url" {
                        in_url_or_sitemap = false;
                        if let Some(loc_str) = current_loc.take() {
                            if let Ok(parsed_url) = Url::parse(&loc_str) {
                                let priority_f32 =
                                    current_priority.take().and_then(|p| p.parse::<f32>().ok());

                                entries.push(SitemapEntry {
                                    loc: parsed_url,
                                    lastmod: current_lastmod.take(),
                                    changefreq: current_changefreq.take(),
                                    priority: priority_f32,
                                });

                                if entries.len() >= max_urls {
                                    break;
                                }
                            }
                        }
                    } else if tag_name == "sitemap" {
                        in_url_or_sitemap = false;
                        if let Some(loc_str) = current_loc.take() {
                            if let Ok(parsed_url) = Url::parse(&loc_str) {
                                index_sitemaps.push(parsed_url);
                                if index_sitemaps.len() >= max_urls {
                                    break;
                                }
                            }
                        }
                    }
                    current_tag.clear();
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    // Tolerant streaming: return what we collected so far if any, else error
                    if entries.is_empty() && index_sitemaps.is_empty() {
                        return Err(CrawlerError::SitemapParseFailed(format!(
                            "XML parsing error at position {}: {e}",
                            reader.buffer_position()
                        )));
                    }
                    break;
                }
                _ => {}
            }
            buf.clear();
        }

        if is_sitemap_index || !index_sitemaps.is_empty() {
            Ok(ParsedSitemapContent::SitemapIndex(index_sitemaps))
        } else {
            Ok(ParsedSitemapContent::Urls(entries))
        }
    }
}
