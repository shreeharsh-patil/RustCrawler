use crate::detect::models::DocumentType;
use crate::document::models::{
    calculate_word_count, compute_blake3_hash, DocumentMetadata, UnifiedDocument,
};
use crate::document::parser::{ContentParser, ParseInput};
use crate::error::CrawlerError;
use crate::models::PageLink;
use async_trait::async_trait;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::json;

pub struct FeedDocumentParser;

impl FeedDocumentParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FeedDocumentParser {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default)]
struct FeedEntry {
    title: Option<String>,
    url: Option<String>,
    published: Option<String>,
    author: Option<String>,
    summary: Option<String>,
    guid: Option<String>,
    categories: Vec<String>,
}

#[async_trait]
impl ContentParser for FeedDocumentParser {
    fn supports(&self, doc_type: DocumentType) -> bool {
        matches!(doc_type, DocumentType::Rss | DocumentType::Atom)
    }

    async fn parse(&self, input: ParseInput<'_>) -> Result<UnifiedDocument, CrawlerError> {
        let max_bytes = input.config.max_xml_size_mb * 1024 * 1024;
        if input.bytes.len() > max_bytes {
            return Err(CrawlerError::DocumentTooLarge(input.bytes.len()));
        }

        let mut reader = Reader::from_reader(input.bytes);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();
        let mut feed_title = None;
        let mut feed_description = None;
        let mut feed_link = None;

        let mut entries = Vec::new();
        let mut current_entry: Option<FeedEntry> = None;
        let mut tag_stack: Vec<String> = Vec::new();
        let mut current_text = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_lowercase();
                    if tag == "item" || tag == "entry" {
                        current_entry = Some(FeedEntry::default());
                    }

                    // Handle Atom <link href="..."/>
                    if tag == "link" {
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_lowercase();
                            if key == "href" {
                                let href_val = String::from_utf8_lossy(&attr.value).to_string();
                                if let Some(ref mut entry) = current_entry {
                                    if entry.url.is_none() {
                                        entry.url = Some(href_val);
                                    }
                                } else if feed_link.is_none() {
                                    feed_link = Some(href_val);
                                }
                            }
                        }
                    }

                    tag_stack.push(tag);
                    current_text.clear();
                }
                Ok(Event::Text(e)) => {
                    let text = match e.unescape() {
                        Ok(t) => t.into_owned(),
                        Err(_) => String::from_utf8_lossy(e.as_ref()).to_string(),
                    };
                    current_text.push_str(&text);
                }
                Ok(Event::CData(e)) => {
                    let text = String::from_utf8_lossy(e.as_ref()).to_string();
                    current_text.push_str(&text);
                }
                Ok(Event::End(e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_lowercase();
                    let trimmed_text = current_text.trim().to_string();

                    if let Some(ref mut entry) = current_entry {
                        match tag.as_str() {
                            "title" => entry.title = Some(trimmed_text),
                            "link" if entry.url.is_none() && !trimmed_text.is_empty() => {
                                entry.url = Some(trimmed_text)
                            }
                            "pubdate" | "published" | "updated" | "dc:date" => {
                                entry.published = Some(trimmed_text)
                            }
                            "author" | "dc:creator" | "name" => entry.author = Some(trimmed_text),
                            "description" | "summary" | "content" => {
                                entry.summary = Some(trimmed_text)
                            }
                            "guid" | "id" => entry.guid = Some(trimmed_text),
                            "category" => entry.categories.push(trimmed_text),
                            "item" | "entry" => {
                                entries.push(current_entry.take().unwrap());
                            }
                            _ => {}
                        }
                    } else {
                        match tag.as_str() {
                            "title" if feed_title.is_none() => feed_title = Some(trimmed_text),
                            "description" | "subtitle" if feed_description.is_none() => {
                                feed_description = Some(trimmed_text)
                            }
                            "link" if feed_link.is_none() && !trimmed_text.is_empty() => {
                                feed_link = Some(trimmed_text)
                            }
                            _ => {}
                        }
                    }

                    tag_stack.pop();
                    current_text.clear();
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(CrawlerError::XmlParseFailed(format!(
                        "Feed parsing error: {e}"
                    )));
                }
                _ => {}
            }
            buf.clear();
        }

        // Build structured JSON and links list
        let mut links = Vec::new();
        let mut json_entries = Vec::new();
        let mut markdown = format!("# {}\n\n", feed_title.as_deref().unwrap_or("Feed"));

        if let Some(ref desc) = feed_description {
            markdown.push_str(&format!("> {desc}\n\n"));
        }

        for entry in &entries {
            let entry_title = entry.title.as_deref().unwrap_or("Untitled Entry");
            let entry_url = entry.url.as_deref().unwrap_or("");

            markdown.push_str(&format!("## {entry_title}\n"));
            if !entry_url.is_empty() {
                markdown.push_str(&format!("- **URL:** {entry_url}\n"));
                if let Ok(parsed) = input.final_url.join(entry_url) {
                    let is_ext = parsed.host_str() != input.final_url.host_str();
                    links.push(PageLink {
                        url: parsed.to_string(),
                        text: entry_title.to_string(),
                        rel: Vec::new(),
                        is_external: is_ext,
                    });
                }
            }
            if let Some(ref pub_date) = entry.published {
                markdown.push_str(&format!("- **Published:** {pub_date}\n"));
            }
            if let Some(ref author) = entry.author {
                markdown.push_str(&format!("- **Author:** {author}\n"));
            }
            if let Some(ref summary) = entry.summary {
                markdown.push_str(&format!("\n{summary}\n\n"));
            }

            json_entries.push(json!({
                "title": entry.title,
                "url": entry.url,
                "published": entry.published,
                "author": entry.author,
                "summary": entry.summary,
                "guid": entry.guid,
                "categories": entry.categories,
            }));
        }

        let structured_data = json!({
            "feed": {
                "title": feed_title,
                "description": feed_description,
                "url": feed_link,
            },
            "entries": json_entries,
        });

        let content_hash = Some(compute_blake3_hash(input.bytes));
        let word_count = Some(calculate_word_count(&markdown));

        let doc_metadata = DocumentMetadata {
            title: feed_title.clone(),
            description: feed_description,
            author: None,
            subject: None,
            creator: None,
            language: None,
            canonical_url: feed_link,
            charset: input.detected.charset.clone().or(Some("utf-8".to_string())),
            mime_type: input.detected.mime_type.clone(),
            content_hash,
            published_time: None,
            modified_time: None,
            favicon: None,
            robots: None,
            generator: None,
            page_count: Some(1),
            row_count: Some(entries.len()),
            word_count,
            open_graph: None,
            twitter: None,
            json_ld: Vec::new(),
            custom: Default::default(),
        };

        Ok(UnifiedDocument {
            source_url: input.url.clone(),
            final_url: input.final_url.clone(),
            document_type: input.detected.document_type,
            title: feed_title,
            text: Some(markdown.clone()),
            markdown: Some(markdown),
            html: None,
            clean_html: None,
            structured_data: Some(structured_data),
            metadata: doc_metadata,
            links,
            images: Vec::new(),
            tables: Vec::new(),
            pages: Vec::new(),
            warnings: Vec::new(),
        })
    }
}
