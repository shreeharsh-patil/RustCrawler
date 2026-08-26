use crate::extraction::models::{ChunkType, ContentChunk};
use crate::models::{ScrapeResult, ScrapeWarning, WarningCode};
use serde_json::Value;
use url::Url;

pub struct ContextSelector;

impl ContextSelector {
    /// Extracts, chunks, scores, and ranks relevant content from one or multiple ScrapeResults.
    /// Returns (selected_chunks, warnings, total_chars_used).
    pub fn select_context(
        results: &[ScrapeResult],
        schema: Option<&Value>,
        prompt: Option<&str>,
        max_context_chars: usize,
        max_chunks: usize,
        max_chunk_chars: usize,
    ) -> (Vec<ContentChunk>, Vec<ScrapeWarning>, usize) {
        let keywords = extract_keywords(schema, prompt);
        let mut all_chunks = Vec::new();

        for result in results {
            let source_url = match Url::parse(&result.url) {
                Ok(u) => u,
                Err(_) => continue,
            };

            let mut page_chunks = chunk_scrape_result(result, &source_url, max_chunk_chars);
            for chunk in &mut page_chunks {
                chunk.score = score_chunk(chunk, &keywords, result);
            }
            all_chunks.extend(page_chunks);
        }

        // Sort chunks descending by score
        all_chunks.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut selected_chunks = Vec::new();
        let mut total_chars = 0;
        let mut warnings = Vec::new();
        let mut truncated = false;

        for chunk in all_chunks {
            if selected_chunks.len() >= max_chunks {
                truncated = true;
                break;
            }

            let chunk_len = chunk.content.len();
            if total_chars + chunk_len > max_context_chars {
                // If we can still fit a partial piece of the best chunk, take what fits if it's the first
                if selected_chunks.is_empty() && max_context_chars > 200 {
                    let mut sliced_chunk = chunk;
                    sliced_chunk.content = sliced_chunk.content[..max_context_chars].to_string();
                    total_chars += sliced_chunk.content.len();
                    selected_chunks.push(sliced_chunk);
                }
                truncated = true;
                break;
            }

            total_chars += chunk_len;
            selected_chunks.push(chunk);
        }

        if truncated {
            warnings.push(ScrapeWarning::new(
                WarningCode::ExtractionContextTruncated,
                format!(
                    "Extraction context truncated to limit of {} characters / {} chunks",
                    max_context_chars, max_chunks
                ),
            ));
        }

        (selected_chunks, warnings, total_chars)
    }
}

fn chunk_scrape_result(
    result: &ScrapeResult,
    source_url: &Url,
    max_chunk_chars: usize,
) -> Vec<ContentChunk> {
    let mut chunks = Vec::new();
    let mut chunk_idx = 0;

    // 1. JSON-LD structured blocks
    if let Some(ref meta) = result.metadata {
        for json_ld in &meta.json_ld {
            if let Ok(json_str) = serde_json::to_string_pretty(json_ld) {
                let bounded_json = if json_str.len() > max_chunk_chars {
                    &json_str[..max_chunk_chars]
                } else {
                    &json_str
                };

                chunk_idx += 1;
                chunks.push(ContentChunk {
                    id: format!("{}_jsonld_{chunk_idx}", source_url.path()),
                    source_url: source_url.clone(),
                    heading_path: vec!["JSON-LD Structured Data".to_string()],
                    content: format!("```json\n{bounded_json}\n```"),
                    content_type: ChunkType::Json,
                    score: 1.0,
                });
            }
        }
    }

    // 2. Tables
    if let Some(ref tables) = result.tables {
        for (t_idx, table) in tables.iter().enumerate() {
            let mut table_md = String::new();
            if !table.headers.is_empty() {
                table_md.push_str(&format!("| {} |\n", table.headers.join(" | ")));
                table_md.push_str(&format!(
                    "| {} |\n",
                    table
                        .headers
                        .iter()
                        .map(|_| "---")
                        .collect::<Vec<_>>()
                        .join(" | ")
                ));
            }
            for row in &table.rows {
                table_md.push_str(&format!("| {} |\n", row.join(" | ")));
                if table_md.len() >= max_chunk_chars {
                    break;
                }
            }

            if !table_md.trim().is_empty() {
                chunk_idx += 1;
                chunks.push(ContentChunk {
                    id: format!("{}_tbl_{t_idx}", source_url.path()),
                    source_url: source_url.clone(),
                    heading_path: vec![format!("Table {}", t_idx + 1)],
                    content: table_md,
                    content_type: ChunkType::Table,
                    score: 0.9,
                });
            }
        }
    }

    // 3. Markdown content split by headings or paragraphs
    if let Some(ref md) = result.markdown {
        let sections = split_markdown_by_headings(md, max_chunk_chars);
        for (heading_path, section_content) in sections {
            if !section_content.trim().is_empty() {
                chunk_idx += 1;
                chunks.push(ContentChunk {
                    id: format!("{}_sec_{chunk_idx}", source_url.path()),
                    source_url: source_url.clone(),
                    heading_path,
                    content: section_content,
                    content_type: ChunkType::Section,
                    score: 0.5,
                });
            }
        }
    } else if let Some(ref text) = result.text {
        // Fallback to text paragraphs
        for para in text.split("\n\n") {
            let trimmed = para.trim();
            if trimmed.len() > 30 {
                chunk_idx += 1;
                let bounded = if trimmed.len() > max_chunk_chars {
                    &trimmed[..max_chunk_chars]
                } else {
                    trimmed
                };
                chunks.push(ContentChunk {
                    id: format!("{}_para_{chunk_idx}", source_url.path()),
                    source_url: source_url.clone(),
                    heading_path: Vec::new(),
                    content: bounded.to_string(),
                    content_type: ChunkType::Paragraph,
                    score: 0.4,
                });
            }
        }
    }

    chunks
}

fn split_markdown_by_headings(md: &str, max_chunk_chars: usize) -> Vec<(Vec<String>, String)> {
    let mut sections = Vec::new();
    let mut current_headings: Vec<String> = Vec::new();
    let mut current_content = String::new();

    for line in md.lines() {
        if line.starts_with("# ") || line.starts_with("## ") || line.starts_with("### ") {
            if !current_content.trim().is_empty() {
                sections.push((current_headings.clone(), current_content.trim().to_string()));
                current_content.clear();
            }

            let heading_text = line.trim_start_matches('#').trim().to_string();
            current_headings = vec![heading_text];
        } else {
            if current_content.len() + line.len() > max_chunk_chars
                && !current_content.trim().is_empty()
            {
                sections.push((current_headings.clone(), current_content.trim().to_string()));
                current_content.clear();
            }
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    if !current_content.trim().is_empty() {
        sections.push((current_headings, current_content.trim().to_string()));
    }

    sections
}

fn extract_keywords(schema: Option<&Value>, prompt: Option<&str>) -> Vec<String> {
    let mut keywords = Vec::new();

    if let Some(p) = prompt {
        for word in p.split_whitespace() {
            let clean = word
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase();
            if clean.len() >= 3 && !is_stopword(&clean) && !keywords.contains(&clean) {
                keywords.push(clean);
            }
        }
    }

    if let Some(s) = schema {
        collect_schema_keywords(s, &mut keywords);
    }

    keywords
}

fn collect_schema_keywords(schema: &Value, keywords: &mut Vec<String>) {
    if let Some(obj) = schema.as_object() {
        if let Some(props) = obj.get("properties").and_then(|p| p.as_object()) {
            for (key, prop_schema) in props {
                let clean = key.to_lowercase();
                if !keywords.contains(&clean) {
                    keywords.push(clean);
                }
                collect_schema_keywords(prop_schema, keywords);
            }
        }
        if let Some(items) = obj.get("items") {
            collect_schema_keywords(items, keywords);
        }
        if let Some(desc) = obj.get("description").and_then(|d| d.as_str()) {
            for word in desc.split_whitespace() {
                let clean = word
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase();
                if clean.len() >= 3 && !is_stopword(&clean) && !keywords.contains(&clean) {
                    keywords.push(clean);
                }
            }
        }
    }
}

fn score_chunk(chunk: &ContentChunk, keywords: &[String], result: &ScrapeResult) -> f32 {
    let mut base_score = match chunk.content_type {
        ChunkType::Json => 1.5,
        ChunkType::Table => 1.2,
        ChunkType::Metadata => 1.0,
        ChunkType::FeedEntry => 0.9,
        ChunkType::Section => 0.8,
        ChunkType::PdfPage | ChunkType::DocxSection => 0.8,
        ChunkType::Paragraph => 0.5,
    };

    let content_lower = chunk.content.to_lowercase();
    let headings_lower = chunk.heading_path.join(" ").to_lowercase();

    let mut keyword_matches = 0;
    for kw in keywords {
        if headings_lower.contains(kw) {
            base_score += 1.0;
            keyword_matches += 1;
        } else if content_lower.contains(kw) {
            base_score += 0.3;
            keyword_matches += 1;
        }
    }

    // Boost if title matches
    if let Some(ref meta) = result.metadata {
        if let Some(ref title) = meta.title {
            let title_lower = title.to_lowercase();
            for kw in keywords {
                if title_lower.contains(kw) {
                    base_score += 0.2;
                }
            }
        }
    }

    base_score + (keyword_matches as f32 * 0.1)
}

fn is_stopword(word: &str) -> bool {
    matches!(
        word,
        "the"
            | "and"
            | "for"
            | "that"
            | "this"
            | "with"
            | "from"
            | "are"
            | "was"
            | "extract"
            | "find"
            | "get"
            | "all"
    )
}
