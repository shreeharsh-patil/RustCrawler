use super::dedupe::compute_chunk_hash;
use super::tokenizer::{ApproximateTokenizer, TokenCounter};
use crate::knowledge::models::{ChunkMetadata, IndexedChunk};
use std::sync::Arc;

/// Configuration for structural chunking
#[derive(Debug, Clone)]
pub struct ChunkingConfig {
    pub target_tokens: usize,
    pub max_tokens: usize,
    pub min_tokens: usize,
    pub overlap_tokens: usize,
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        Self {
            target_tokens: 500,
            max_tokens: 800,
            min_tokens: 100,
            overlap_tokens: 80,
        }
    }
}

/// Structural semantic chunker that respects document structure
pub struct StructuralChunker {
    config: ChunkingConfig,
    tokenizer: Arc<dyn TokenCounter>,
}

impl StructuralChunker {
    pub fn new(config: ChunkingConfig, tokenizer: Arc<dyn TokenCounter>) -> Self {
        Self { config, tokenizer }
    }

    pub fn with_default_tokenizer(config: ChunkingConfig) -> Self {
        Self {
            config,
            tokenizer: Arc::new(ApproximateTokenizer),
        }
    }

    /// Chunks a markdown document into semantic chunks preserving heading hierarchy,
    /// code blocks, tables, lists, and paragraphs.
    pub fn chunk_markdown(
        &self,
        document_id: &str,
        source_url: Option<&str>,
        markdown: &str,
    ) -> Vec<IndexedChunk> {
        let blocks = parse_markdown_blocks(markdown);
        let mut chunks = Vec::new();
        let mut current_heading_path: Vec<String> = Vec::new();
        let mut current_block_group: Vec<ParsedBlock> = Vec::new();
        let mut current_tokens = 0;
        let mut position = 0u32;

        for block in blocks {
            match block {
                ParsedBlock::Heading { level, text } => {
                    // Flush existing accumulated blocks before changing heading context
                    if !current_block_group.is_empty() {
                        let chunk = self.build_chunk_from_blocks(
                            document_id,
                            source_url,
                            &current_block_group,
                            &current_heading_path,
                            position,
                        );
                        chunks.push(chunk);
                        position += 1;
                        current_block_group.clear();
                        current_tokens = 0;
                    }

                    // Update heading hierarchy
                    let heading_clean = text.trim().to_string();
                    let target_depth = level as usize;
                    if current_heading_path.len() >= target_depth {
                        current_heading_path.truncate(target_depth.saturating_sub(1));
                    }
                    current_heading_path.push(heading_clean);
                }
                ParsedBlock::CodeBlock { lang, code } => {
                    let block_tokens = self.tokenizer.count(&code);
                    if block_tokens > self.config.max_tokens {
                        // Split oversized code block by lines
                        if !current_block_group.is_empty() {
                            let chunk = self.build_chunk_from_blocks(
                                document_id,
                                source_url,
                                &current_block_group,
                                &current_heading_path,
                                position,
                            );
                            chunks.push(chunk);
                            position += 1;
                            current_block_group.clear();
                            current_tokens = 0;
                        }
                        for sub_code in
                            split_oversized_code(&code, self.config.target_tokens, &*self.tokenizer)
                        {
                            let meta = ChunkMetadata {
                                code_lang: lang.clone(),
                                ..Default::default()
                            };
                            let tokens = self.tokenizer.count(&sub_code);
                            let hash = compute_chunk_hash(&sub_code);
                            let chunk_id = format!(
                                "chk_{}_{}_{}",
                                document_id,
                                position,
                                &hash[..8.min(hash.len())]
                            );
                            chunks.push(IndexedChunk {
                                chunk_id,
                                document_id: document_id.to_string(),
                                source_url: source_url.map(ToString::to_string),
                                heading_path: current_heading_path.clone(),
                                text: sub_code,
                                token_count: tokens,
                                content_hash: hash,
                                position,
                                metadata: meta,
                            });
                            position += 1;
                        }
                    } else if current_tokens + block_tokens > self.config.max_tokens {
                        // Flush accumulated group and start new group with code block
                        if !current_block_group.is_empty() {
                            let chunk = self.build_chunk_from_blocks(
                                document_id,
                                source_url,
                                &current_block_group,
                                &current_heading_path,
                                position,
                            );
                            chunks.push(chunk);
                            position += 1;
                            current_block_group.clear();
                        }
                        current_block_group.push(ParsedBlock::CodeBlock { lang, code });
                        current_tokens = block_tokens;
                    } else {
                        current_tokens += block_tokens;
                        current_block_group.push(ParsedBlock::CodeBlock { lang, code });
                    }
                }
                ParsedBlock::Table { table_text } => {
                    let block_tokens = self.tokenizer.count(&table_text);
                    if current_tokens + block_tokens > self.config.max_tokens
                        && !current_block_group.is_empty()
                    {
                        let chunk = self.build_chunk_from_blocks(
                            document_id,
                            source_url,
                            &current_block_group,
                            &current_heading_path,
                            position,
                        );
                        chunks.push(chunk);
                        position += 1;
                        current_block_group.clear();
                        current_tokens = 0;
                    }
                    current_tokens += block_tokens;
                    current_block_group.push(ParsedBlock::Table { table_text });
                }
                ParsedBlock::Paragraph(p_text) => {
                    let p_tokens = self.tokenizer.count(&p_text);
                    if p_tokens > self.config.max_tokens {
                        // Split oversized paragraph by sentences
                        if !current_block_group.is_empty() {
                            let chunk = self.build_chunk_from_blocks(
                                document_id,
                                source_url,
                                &current_block_group,
                                &current_heading_path,
                                position,
                            );
                            chunks.push(chunk);
                            position += 1;
                            current_block_group.clear();
                            current_tokens = 0;
                        }
                        for sentence_chunk in split_oversized_text(
                            &p_text,
                            self.config.target_tokens,
                            &*self.tokenizer,
                        ) {
                            let tokens = self.tokenizer.count(&sentence_chunk);
                            let hash = compute_chunk_hash(&sentence_chunk);
                            let chunk_id = format!(
                                "chk_{}_{}_{}",
                                document_id,
                                position,
                                &hash[..8.min(hash.len())]
                            );
                            chunks.push(IndexedChunk {
                                chunk_id,
                                document_id: document_id.to_string(),
                                source_url: source_url.map(ToString::to_string),
                                heading_path: current_heading_path.clone(),
                                text: sentence_chunk,
                                token_count: tokens,
                                content_hash: hash,
                                position,
                                metadata: ChunkMetadata::default(),
                            });
                            position += 1;
                        }
                    } else if current_tokens + p_tokens > self.config.target_tokens
                        && current_tokens >= self.config.min_tokens
                    {
                        // Flush accumulated group and start new group
                        let chunk = self.build_chunk_from_blocks(
                            document_id,
                            source_url,
                            &current_block_group,
                            &current_heading_path,
                            position,
                        );
                        chunks.push(chunk);
                        position += 1;
                        current_block_group.clear();
                        current_block_group.push(ParsedBlock::Paragraph(p_text));
                        current_tokens = p_tokens;
                    } else {
                        current_tokens += p_tokens;
                        current_block_group.push(ParsedBlock::Paragraph(p_text));
                    }
                }
                ParsedBlock::List(list_text) => {
                    let l_tokens = self.tokenizer.count(&list_text);
                    if current_tokens + l_tokens > self.config.max_tokens
                        && !current_block_group.is_empty()
                    {
                        let chunk = self.build_chunk_from_blocks(
                            document_id,
                            source_url,
                            &current_block_group,
                            &current_heading_path,
                            position,
                        );
                        chunks.push(chunk);
                        position += 1;
                        current_block_group.clear();
                        current_tokens = 0;
                    }
                    current_tokens += l_tokens;
                    current_block_group.push(ParsedBlock::List(list_text));
                }
            }
        }

        // Flush any remaining blocks
        if !current_block_group.is_empty() {
            let chunk = self.build_chunk_from_blocks(
                document_id,
                source_url,
                &current_block_group,
                &current_heading_path,
                position,
            );
            chunks.push(chunk);
        }

        chunks
    }

    /// Chunks a structured JSON value preserving json property paths
    pub fn chunk_json(
        &self,
        document_id: &str,
        source_url: Option<&str>,
        json_val: &serde_json::Value,
    ) -> Vec<IndexedChunk> {
        let mut chunks = Vec::new();
        let mut position = 0u32;

        match json_val {
            serde_json::Value::Array(arr) => {
                let mut current_items = Vec::new();
                let mut current_tokens = 0;
                for (idx, item) in arr.iter().enumerate() {
                    let item_str = serde_json::to_string_pretty(item).unwrap_or_default();
                    let tokens = self.tokenizer.count(&item_str);
                    if current_tokens + tokens > self.config.target_tokens
                        && !current_items.is_empty()
                    {
                        let combined_text = current_items.join("\n\n");
                        let hash = compute_chunk_hash(&combined_text);
                        let chunk_id = format!(
                            "chk_{}_{}_{}",
                            document_id,
                            position,
                            &hash[..8.min(hash.len())]
                        );
                        let meta = ChunkMetadata {
                            json_path: Some(format!("$[0..{}]", idx)),
                            ..Default::default()
                        };
                        chunks.push(IndexedChunk {
                            chunk_id,
                            document_id: document_id.to_string(),
                            source_url: source_url.map(ToString::to_string),
                            heading_path: vec!["Root Array".to_string()],
                            text: combined_text,
                            token_count: current_tokens,
                            content_hash: hash,
                            position,
                            metadata: meta,
                        });
                        position += 1;
                        current_items.clear();
                        current_tokens = 0;
                    }
                    current_items.push(format!("// Item [{idx}]\n{item_str}"));
                    current_tokens += tokens;
                }
                if !current_items.is_empty() {
                    let combined_text = current_items.join("\n\n");
                    let hash = compute_chunk_hash(&combined_text);
                    let chunk_id = format!(
                        "chk_{}_{}_{}",
                        document_id,
                        position,
                        &hash[..8.min(hash.len())]
                    );
                    let meta = ChunkMetadata {
                        json_path: Some(format!("$[..{}]", arr.len())),
                        ..Default::default()
                    };
                    chunks.push(IndexedChunk {
                        chunk_id,
                        document_id: document_id.to_string(),
                        source_url: source_url.map(ToString::to_string),
                        heading_path: vec!["Root Array".to_string()],
                        text: combined_text,
                        token_count: current_tokens,
                        content_hash: hash,
                        position,
                        metadata: meta,
                    });
                }
            }
            serde_json::Value::Object(obj) => {
                let mut current_pairs = Vec::new();
                let mut current_tokens = 0;
                for (key, val) in obj {
                    let pair_str = format!(
                        "\"{}\": {}",
                        key,
                        serde_json::to_string_pretty(val).unwrap_or_default()
                    );
                    let tokens = self.tokenizer.count(&pair_str);
                    if current_tokens + tokens > self.config.target_tokens
                        && !current_pairs.is_empty()
                    {
                        let combined_text = format!("{{\n  {}\n}}", current_pairs.join(",\n  "));
                        let hash = compute_chunk_hash(&combined_text);
                        let chunk_id = format!(
                            "chk_{}_{}_{}",
                            document_id,
                            position,
                            &hash[..8.min(hash.len())]
                        );
                        let meta = ChunkMetadata {
                            json_path: Some(format!("$.{}", key)),
                            ..Default::default()
                        };
                        chunks.push(IndexedChunk {
                            chunk_id,
                            document_id: document_id.to_string(),
                            source_url: source_url.map(ToString::to_string),
                            heading_path: vec!["JSON Object".to_string()],
                            text: combined_text,
                            token_count: current_tokens,
                            content_hash: hash,
                            position,
                            metadata: meta,
                        });
                        position += 1;
                        current_pairs.clear();
                        current_tokens = 0;
                    }
                    current_pairs.push(pair_str);
                    current_tokens += tokens;
                }
                if !current_pairs.is_empty() {
                    let combined_text = format!("{{\n  {}\n}}", current_pairs.join(",\n  "));
                    let hash = compute_chunk_hash(&combined_text);
                    let chunk_id = format!(
                        "chk_{}_{}_{}",
                        document_id,
                        position,
                        &hash[..8.min(hash.len())]
                    );
                    chunks.push(IndexedChunk {
                        chunk_id,
                        document_id: document_id.to_string(),
                        source_url: source_url.map(ToString::to_string),
                        heading_path: vec!["JSON Object".to_string()],
                        text: combined_text,
                        token_count: current_tokens,
                        content_hash: hash,
                        position,
                        metadata: ChunkMetadata::default(),
                    });
                }
            }
            _ => {
                let text = json_val.to_string();
                let tokens = self.tokenizer.count(&text);
                let hash = compute_chunk_hash(&text);
                let chunk_id = format!(
                    "chk_{}_{}_{}",
                    document_id,
                    position,
                    &hash[..8.min(hash.len())]
                );
                chunks.push(IndexedChunk {
                    chunk_id,
                    document_id: document_id.to_string(),
                    source_url: source_url.map(ToString::to_string),
                    heading_path: Vec::new(),
                    text,
                    token_count: tokens,
                    content_hash: hash,
                    position,
                    metadata: ChunkMetadata::default(),
                });
            }
        }

        chunks
    }

    /// Chunks PDF extracted page text preserving page numbers
    pub fn chunk_pdf_pages(
        &self,
        document_id: &str,
        source_url: Option<&str>,
        pages: &[(usize, String)], // (page_number, page_text)
    ) -> Vec<IndexedChunk> {
        let mut chunks = Vec::new();
        let mut position = 0u32;

        for (page_num, text) in pages {
            if text.trim().is_empty() {
                continue;
            }
            let page_chunks = self.chunk_markdown(document_id, source_url, text);
            for mut chunk in page_chunks {
                chunk.metadata.page_number = Some(*page_num);
                chunk.position = position;
                let hash = compute_chunk_hash(&chunk.text);
                chunk.chunk_id = format!(
                    "chk_{}_{}_{}",
                    document_id,
                    position,
                    &hash[..8.min(hash.len())]
                );
                chunks.push(chunk);
                position += 1;
            }
        }

        chunks
    }

    fn build_chunk_from_blocks(
        &self,
        document_id: &str,
        source_url: Option<&str>,
        blocks: &[ParsedBlock],
        heading_path: &[String],
        position: u32,
    ) -> IndexedChunk {
        let mut texts = Vec::with_capacity(blocks.len());
        let mut meta = ChunkMetadata::default();

        for b in blocks {
            match b {
                ParsedBlock::Heading { text, .. } => {
                    texts.push(text.clone());
                }
                ParsedBlock::CodeBlock { lang, code } => {
                    if meta.code_lang.is_none() {
                        meta.code_lang = lang.clone();
                    }
                    if let Some(l) = lang {
                        texts.push(format!("```{l}\n{code}\n```"));
                    } else {
                        texts.push(format!("```\n{code}\n```"));
                    }
                }
                ParsedBlock::Table { table_text } => {
                    texts.push(table_text.clone());
                }
                ParsedBlock::Paragraph(p) => {
                    texts.push(p.clone());
                }
                ParsedBlock::List(l) => {
                    texts.push(l.clone());
                }
            }
        }

        let combined_text = texts.join("\n\n");
        let token_count = self.tokenizer.count(&combined_text);
        let content_hash = compute_chunk_hash(&combined_text);
        let chunk_id = format!(
            "chk_{}_{}_{}",
            document_id,
            position,
            &content_hash[..8.min(content_hash.len())]
        );

        IndexedChunk {
            chunk_id,
            document_id: document_id.to_string(),
            source_url: source_url.map(ToString::to_string),
            heading_path: heading_path.to_vec(),
            text: combined_text,
            token_count,
            content_hash,
            position,
            metadata: meta,
        }
    }
}

#[derive(Debug, Clone)]
enum ParsedBlock {
    Heading { level: u8, text: String },
    CodeBlock { lang: Option<String>, code: String },
    Table { table_text: String },
    Paragraph(String),
    List(String),
}

fn parse_markdown_blocks(markdown: &str) -> Vec<ParsedBlock> {
    let mut blocks = Vec::new();
    let mut lines = markdown.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Headings (# H1, ## H2, etc.)
        if trimmed.starts_with('#') {
            let hash_count = trimmed.chars().take_while(|c| *c == '#').count();
            if hash_count <= 6 && trimmed[hash_count..].starts_with(' ') {
                let heading_text = trimmed[hash_count..].trim().to_string();
                blocks.push(ParsedBlock::Heading {
                    level: hash_count as u8,
                    text: heading_text,
                });
                continue;
            }
        }

        // Code blocks (``` or ~~~)
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let delimiter = &trimmed[..3];
            let lang = trimmed[3..].trim();
            let lang_opt = if lang.is_empty() {
                None
            } else {
                Some(lang.to_string())
            };
            let mut code_lines = Vec::new();
            for code_line in lines.by_ref() {
                if code_line.trim().starts_with(delimiter) {
                    break;
                }
                code_lines.push(code_line);
            }
            blocks.push(ParsedBlock::CodeBlock {
                lang: lang_opt,
                code: code_lines.join("\n"),
            });
            continue;
        }

        // Tables (| header |)
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let mut table_lines = vec![line];
            while let Some(next_line) = lines.peek() {
                let next_trimmed = next_line.trim();
                if next_trimmed.starts_with('|') && next_trimmed.ends_with('|') {
                    table_lines.push(lines.next().unwrap());
                } else {
                    break;
                }
            }
            blocks.push(ParsedBlock::Table {
                table_text: table_lines.join("\n"),
            });
            continue;
        }

        // Lists (*, -, +, 1.)
        if trimmed.starts_with("* ")
            || trimmed.starts_with("- ")
            || trimmed.starts_with("+ ")
            || is_ordered_list_item(trimmed)
        {
            let mut list_lines = vec![line];
            while let Some(next_line) = lines.peek() {
                let next_trimmed = next_line.trim();
                if next_trimmed.starts_with("* ")
                    || next_trimmed.starts_with("- ")
                    || next_trimmed.starts_with("+ ")
                    || is_ordered_list_item(next_trimmed)
                    || (next_line.starts_with("  ") && !next_trimmed.is_empty())
                {
                    list_lines.push(lines.next().unwrap());
                } else {
                    break;
                }
            }
            blocks.push(ParsedBlock::List(list_lines.join("\n")));
            continue;
        }

        // Regular paragraph (accumulate contiguous lines until empty line or block start)
        let mut p_lines = vec![line];
        while let Some(next_line) = lines.peek() {
            let next_trimmed = next_line.trim();
            if next_trimmed.is_empty()
                || next_trimmed.starts_with('#')
                || next_trimmed.starts_with("```")
                || next_trimmed.starts_with("~~~")
                || (next_trimmed.starts_with('|') && next_trimmed.ends_with('|'))
                || next_trimmed.starts_with("* ")
                || next_trimmed.starts_with("- ")
                || next_trimmed.starts_with("+ ")
                || is_ordered_list_item(next_trimmed)
            {
                break;
            }
            p_lines.push(lines.next().unwrap());
        }
        blocks.push(ParsedBlock::Paragraph(p_lines.join(" ")));
    }

    blocks
}

fn is_ordered_list_item(s: &str) -> bool {
    let mut chars = s.chars();
    let mut digit_count = 0;
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            digit_count += 1;
        } else if c == '.' && digit_count > 0 {
            return chars.next() == Some(' ');
        } else {
            return false;
        }
    }
    false
}

fn split_oversized_text(
    text: &str,
    target_tokens: usize,
    tokenizer: &dyn TokenCounter,
) -> Vec<String> {
    let mut chunks = Vec::new();
    let sentences: Vec<&str> = text
        .split_inclusive(&['.', '!', '?', ';', '\n'][..])
        .collect();
    let mut current_chunk = String::new();
    let mut current_tokens = 0;

    for s in sentences {
        let s_tokens = tokenizer.count(s);
        if current_tokens + s_tokens > target_tokens && !current_chunk.is_empty() {
            chunks.push(current_chunk.trim().to_string());
            current_chunk = String::new();
            current_tokens = 0;
        }
        current_chunk.push_str(s);
        current_tokens += s_tokens;
    }

    if !current_chunk.trim().is_empty() {
        chunks.push(current_chunk.trim().to_string());
    }

    if chunks.is_empty() {
        vec![text.to_string()]
    } else {
        chunks
    }
}

fn split_oversized_code(
    code: &str,
    target_tokens: usize,
    tokenizer: &dyn TokenCounter,
) -> Vec<String> {
    let mut chunks = Vec::new();
    let lines: Vec<&str> = code.lines().collect();
    let mut current_lines = Vec::new();
    let mut current_tokens = 0;

    for l in lines {
        let l_tokens = tokenizer.count(l);
        if current_tokens + l_tokens > target_tokens && !current_lines.is_empty() {
            chunks.push(current_lines.join("\n"));
            current_lines.clear();
            current_tokens = 0;
        }
        current_lines.push(l);
        current_tokens += l_tokens;
    }

    if !current_lines.is_empty() {
        chunks.push(current_lines.join("\n"));
    }

    if chunks.is_empty() {
        vec![code.to_string()]
    } else {
        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_structural_chunker_markdown_headings_and_tables() {
        let markdown = r#"# Main Title

This is the introductory paragraph for the document.

## Section 1: Features

Here are the key features of RustCrawler:
* Blazing fast async crawling
* Smart JS rendering
* Universal document parsing

### Deep Dive: Performance

| Metric | Target | Actual |
|---|---|---|
| Latency | < 50ms | 12ms |
| Memory | < 50MB | 24MB |

```rust
fn execute_crawl() -> Result<(), CrawlerError> {
    println!("Crawling with zero overhead");
    Ok(())
}
```
"#;

        let chunker = StructuralChunker::with_default_tokenizer(ChunkingConfig {
            target_tokens: 100,
            max_tokens: 200,
            min_tokens: 10,
            overlap_tokens: 0,
        });

        let chunks = chunker.chunk_markdown("doc_1", Some("https://example.com/docs"), markdown);
        assert!(!chunks.is_empty(), "Should generate chunks");

        // Verify heading hierarchy preservation
        let deep_chunk = chunks
            .iter()
            .find(|c| c.text.contains("Metric"))
            .expect("Table chunk should exist");
        assert_eq!(
            deep_chunk.heading_path,
            vec![
                "Main Title",
                "Section 1: Features",
                "Deep Dive: Performance"
            ]
        );

        // Verify code block is parsed
        let code_chunk = chunks
            .iter()
            .find(|c| c.text.contains("execute_crawl"))
            .expect("Code chunk should exist");
        assert_eq!(code_chunk.metadata.code_lang, Some("rust".to_string()));
    }

    #[test]
    fn test_structural_chunker_ordered_lists() {
        let markdown = r#"# Getting Started

Follow these installation steps:
1. Clone the repository with git.
2. Run cargo build --release.
3. Start the server with ./target/release/rustcrawl serve.
"#;

        let chunker = StructuralChunker::with_default_tokenizer(ChunkingConfig {
            target_tokens: 100,
            max_tokens: 200,
            min_tokens: 10,
            overlap_tokens: 0,
        });

        let chunks = chunker.chunk_markdown("doc_2", None, markdown);
        assert!(!chunks.is_empty());
        assert!(chunks.iter().any(|c| c.text.contains("1. Clone the repository")));
    }
}
