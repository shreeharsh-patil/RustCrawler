use crate::crawl::dedupe::normalize_crawl_url;
use scraper::{Html, Selector};
use url::Url;

#[derive(Debug, Clone)]
pub struct LightweightPageSummary {
    pub title: Option<String>,
    pub description: Option<String>,
    pub links: Vec<(Url, Option<String>)>,
}

pub struct LightweightExtractor;

impl LightweightExtractor {
    /// Extracts title, meta description, and valid outbound links directly from HTML without full parsing.
    pub fn extract(
        html_content: &str,
        base_url: &Url,
        max_links: usize,
        max_anchor_length: usize,
    ) -> LightweightPageSummary {
        let document = Html::parse_document(html_content);

        // 1. Extract title
        let title = Selector::parse("title")
            .ok()
            .and_then(|sel| document.select(&sel).next())
            .map(|el| el.text().collect::<String>().trim().to_string())
            .filter(|t| !t.is_empty());

        // 2. Extract description (meta description or og:description)
        let mut description = None;
        if let Ok(meta_sel) =
            Selector::parse("meta[name='description'], meta[property='og:description']")
        {
            for el in document.select(&meta_sel) {
                if let Some(content) = el.value().attr("content") {
                    let trimmed = content.trim().to_string();
                    if !trimmed.is_empty() {
                        description = Some(trimmed);
                        break;
                    }
                }
            }
        }

        // 3. Extract links and anchor text
        let mut links = Vec::new();
        if let Ok(a_sel) = Selector::parse("a[href]") {
            for el in document.select(&a_sel) {
                if links.len() >= max_links {
                    break;
                }

                if let Some(href) = el.value().attr("href") {
                    let trimmed_href = href.trim();
                    if trimmed_href.is_empty()
                        || trimmed_href.starts_with('#')
                        || trimmed_href.starts_with("javascript:")
                        || trimmed_href.starts_with("mailto:")
                        || trimmed_href.starts_with("tel:")
                    {
                        continue;
                    }

                    if let Ok(joined) = base_url.join(trimmed_href) {
                        let normalized_str = normalize_crawl_url(&joined, false, true);
                        let normalized = Url::parse(&normalized_str).unwrap_or(joined);
                        let anchor = el.text().collect::<String>().trim().to_string();
                        let anchor_opt = if anchor.is_empty() {
                            None
                        } else {
                            Some(if anchor.len() > max_anchor_length {
                                anchor[..max_anchor_length].to_string()
                            } else {
                                anchor
                            })
                        };

                        links.push((normalized, anchor_opt));
                    }
                }
            }
        }

        LightweightPageSummary {
            title,
            description,
            links,
        }
    }
}
