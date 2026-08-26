use crate::utils::urls::resolve_relative_url;
use ego_tree::NodeRef;
use scraper::node::Node;
use scraper::{Html, Node as ScraperNode};
use url::Url;

/// Converts cleaned or raw HTML into clean, high-quality Markdown.
pub fn html_to_markdown(html_content: &str, base_url: &Url) -> String {
    let parsed = Html::parse_fragment(html_content);
    let mut context = MarkdownContext {
        base_url,
        list_depth: 0,
        list_indices: Vec::new(),
        inside_pre: false,
    };

    let mut output = String::with_capacity(html_content.len());
    for child in parsed.tree.root().children() {
        render_node(&child, &mut output, &mut context);
    }

    clean_markdown_whitespace(&output)
}

struct MarkdownContext<'a> {
    base_url: &'a Url,
    list_depth: usize,
    list_indices: Vec<usize>,
    inside_pre: bool,
}

fn render_node(node: &NodeRef<Node>, out: &mut String, ctx: &mut MarkdownContext) {
    match node.value() {
        ScraperNode::Text(text) => {
            if ctx.inside_pre {
                out.push_str(text);
            } else {
                let text_str: &str = text;
                // Avoid extra spaces when adjacent to whitespace
                let mut prev_space = out.ends_with(' ') || out.ends_with('\n');
                for c in text_str.chars() {
                    if c.is_whitespace() {
                        if !prev_space {
                            out.push(' ');
                            prev_space = true;
                        }
                    } else {
                        out.push(c);
                        prev_space = false;
                    }
                }
            }
        }
        ScraperNode::Element(elem) => {
            let tag = elem.name().to_lowercase();

            match tag.as_str() {
                // Headings
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let level = tag[1..].parse::<usize>().unwrap_or(1);
                    ensure_newline(out, 2);
                    out.push_str(&"#".repeat(level));
                    out.push(' ');
                    render_children(node, out, ctx);
                    ensure_newline(out, 2);
                }

                // Paragraphs
                "p" => {
                    ensure_newline(out, 2);
                    render_children(node, out, ctx);
                    ensure_newline(out, 2);
                }

                // Inline emphasis
                "b" | "strong" => {
                    out.push_str("**");
                    render_children(node, out, ctx);
                    out.push_str("**");
                }
                "i" | "em" => {
                    out.push('*');
                    render_children(node, out, ctx);
                    out.push('*');
                }
                "s" | "del" | "strike" => {
                    out.push_str("~~");
                    render_children(node, out, ctx);
                    out.push_str("~~");
                }

                // Code and Preformatted blocks
                "pre" => {
                    ensure_newline(out, 2);
                    let mut lang = "";
                    for child in node.children() {
                        if let ScraperNode::Element(ref child_elem) = child.value() {
                            if child_elem.name() == "code" {
                                if let Some(class_attr) = child_elem.attr("class") {
                                    for class in class_attr.split_whitespace() {
                                        if let Some(l) = class.strip_prefix("language-") {
                                            lang = l;
                                        } else if let Some(l) = class.strip_prefix("lang-") {
                                            lang = l;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    out.push_str("```");
                    out.push_str(lang);
                    out.push('\n');

                    let prev_pre = ctx.inside_pre;
                    ctx.inside_pre = true;
                    render_children(node, out, ctx);
                    ctx.inside_pre = prev_pre;

                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str("```\n\n");
                }

                "code" => {
                    if ctx.inside_pre {
                        render_children(node, out, ctx);
                    } else {
                        out.push('`');
                        render_children(node, out, ctx);
                        out.push('`');
                    }
                }

                // Links
                "a" => {
                    let href = elem.attr("href").map(|s| s.trim());
                    let mut link_text = String::new();
                    render_children(node, &mut link_text, ctx);
                    let link_text_trimmed = link_text.trim();

                    if let Some(raw_href) = href {
                        if !raw_href.is_empty() {
                            let resolved = resolve_relative_url(ctx.base_url, raw_href)
                                .map(|u| u.to_string())
                                .unwrap_or_else(|| raw_href.to_string());

                            let display_text = if link_text_trimmed.is_empty() {
                                &resolved
                            } else {
                                link_text_trimmed
                            };

                            out.push('[');
                            out.push_str(display_text);
                            out.push_str("](");
                            out.push_str(&resolved);
                            out.push(')');
                            return;
                        }
                    }

                    out.push_str(&link_text);
                }

                // Images
                "img" => {
                    let src = elem
                        .attr("src")
                        .or_else(|| elem.attr("data-src"))
                        .map(|s| s.trim())
                        .unwrap_or("");
                    let alt = elem.attr("alt").map(|s| s.trim()).unwrap_or("");

                    if !src.is_empty() {
                        let resolved = resolve_relative_url(ctx.base_url, src)
                            .map(|u| u.to_string())
                            .unwrap_or_else(|| src.to_string());

                        out.push_str("![");
                        out.push_str(alt);
                        out.push_str("](");
                        out.push_str(&resolved);
                        out.push(')');
                    }
                }

                // Lists
                "ul" => {
                    ensure_newline(out, 1);
                    ctx.list_depth += 1;
                    ctx.list_indices.push(0); // 0 indicates unordered
                    render_children(node, out, ctx);
                    ctx.list_indices.pop();
                    ctx.list_depth -= 1;
                    ensure_newline(out, 1);
                }

                "ol" => {
                    ensure_newline(out, 1);
                    ctx.list_depth += 1;
                    ctx.list_indices.push(1); // Starting counter for ordered
                    render_children(node, out, ctx);
                    ctx.list_indices.pop();
                    ctx.list_depth -= 1;
                    ensure_newline(out, 1);
                }

                "li" => {
                    ensure_newline(out, 1);
                    let indent = "  ".repeat(ctx.list_depth.saturating_sub(1));
                    out.push_str(&indent);

                    let list_type = ctx.list_indices.last_mut();
                    match list_type {
                        Some(0) => {
                            out.push_str("- ");
                        }
                        Some(idx) => {
                            out.push_str(&format!("{idx}. "));
                            *idx += 1;
                        }
                        None => {
                            out.push_str("- ");
                        }
                    }

                    render_children(node, out, ctx);
                    ensure_newline(out, 1);
                }

                // Blockquotes
                "blockquote" => {
                    ensure_newline(out, 2);
                    let mut quote_content = String::new();
                    render_children(node, &mut quote_content, ctx);

                    for line in quote_content.lines() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            out.push_str("> ");
                            out.push_str(trimmed);
                            out.push('\n');
                        }
                    }
                    ensure_newline(out, 2);
                }

                // Tables
                "table" => {
                    ensure_newline(out, 2);
                    render_table(node, out, ctx);
                    ensure_newline(out, 2);
                }

                // Line breaks and horizontal rules
                "br" => {
                    out.push('\n');
                }
                "hr" => {
                    ensure_newline(out, 2);
                    out.push_str("---");
                    ensure_newline(out, 2);
                }

                // Default container tags (div, section, article, span, etc.)
                _ => {
                    render_children(node, out, ctx);
                }
            }
        }
        _ => {}
    }
}

fn render_children(node: &NodeRef<Node>, out: &mut String, ctx: &mut MarkdownContext) {
    for child in node.children() {
        render_node(&child, out, ctx);
    }
}

fn render_table(table_node: &NodeRef<Node>, out: &mut String, ctx: &mut MarkdownContext) {
    let mut rows: Vec<Vec<String>> = Vec::new();
    collect_table_rows(table_node, &mut rows, ctx);

    if rows.is_empty() {
        return;
    }

    let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if max_cols == 0 {
        return;
    }

    // Pad each row to max_cols
    for row in &mut rows {
        while row.len() < max_cols {
            row.push(String::new());
        }
    }

    // First row as header
    let header = &rows[0];
    out.push_str("| ");
    out.push_str(&header.join(" | "));
    out.push_str(" |\n");

    // Divider row
    out.push_str("| ");
    let dividers: Vec<&str> = (0..max_cols).map(|_| "---").collect();
    out.push_str(&dividers.join(" | "));
    out.push_str(" |\n");

    // Data rows
    for row in &rows[1..] {
        out.push_str("| ");
        out.push_str(&row.join(" | "));
        out.push_str(" |\n");
    }
}

fn collect_table_rows(
    node: &NodeRef<Node>,
    rows: &mut Vec<Vec<String>>,
    ctx: &mut MarkdownContext,
) {
    if let ScraperNode::Element(elem) = node.value() {
        if elem.name() == "tr" {
            let mut row = Vec::new();
            for child in node.children() {
                if let ScraperNode::Element(c_elem) = child.value() {
                    if c_elem.name() == "th" || c_elem.name() == "td" {
                        let mut cell_text = String::new();
                        render_children(&child, &mut cell_text, ctx);
                        let clean_cell = cell_text
                            .replace('|', "\\|")
                            .replace('\n', " ")
                            .trim()
                            .to_string();
                        row.push(clean_cell);
                    }
                }
            }
            if !row.is_empty() {
                rows.push(row);
            }
            return;
        }
    }

    for child in node.children() {
        collect_table_rows(&child, rows, ctx);
    }
}

fn ensure_newline(out: &mut String, count: usize) {
    let mut existing = 0;
    for c in out.chars().rev() {
        if c == '\n' {
            existing += 1;
        } else {
            break;
        }
    }

    for _ in existing..count {
        out.push('\n');
    }
}

fn clean_markdown_whitespace(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut consecutive_newlines = 0;

    for line in input.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            consecutive_newlines += 1;
            if consecutive_newlines <= 2 {
                result.push('\n');
            }
        } else {
            consecutive_newlines = 0;
            result.push_str(trimmed);
            result.push('\n');
        }
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_to_markdown_headings_and_paragraphs() {
        let html = r#"
            <h1>Main Title</h1>
            <p>This is a <b>bold</b> and <i>italic</i> paragraph.</p>
            <h2>Subtitle</h2>
            <p>Another paragraph with <a href="/docs">Docs Link</a>.</p>
        "#;
        let base = Url::parse("https://example.com/sub/").unwrap();
        let md = html_to_markdown(html, &base);

        assert!(md.contains("# Main Title"));
        assert!(md.contains("This is a **bold** and *italic* paragraph."));
        assert!(md.contains("## Subtitle"));
        assert!(md.contains("[Docs Link](https://example.com/docs)"));
    }

    #[test]
    fn test_html_to_markdown_lists() {
        let html = r#"
            <ul>
                <li>Item 1</li>
                <li>Item 2
                    <ul>
                        <li>Subitem 2.1</li>
                    </ul>
                </li>
            </ul>
            <ol>
                <li>First</li>
                <li>Second</li>
            </ol>
        "#;
        let base = Url::parse("https://example.com").unwrap();
        let md = html_to_markdown(html, &base);

        assert!(md.contains("- Item 1"));
        assert!(md.contains("- Item 2"));
        assert!(md.contains("  - Subitem 2.1"));
        assert!(md.contains("1. First"));
        assert!(md.contains("2. Second"));
    }

    #[test]
    fn test_html_to_markdown_code_block() {
        let html = r#"
            <p>Here is some <code>inline_code()</code>.</p>
            <pre><code class="language-rust">fn main() {
    println!("Hello, World!");
}</code></pre>
        "#;
        let base = Url::parse("https://example.com").unwrap();
        let md = html_to_markdown(html, &base);

        assert!(md.contains("`inline_code()`"));
        assert!(md.contains("```rust\nfn main() {\n    println!(\"Hello, World!\");\n}\n```"));
    }

    #[test]
    fn test_html_to_markdown_table() {
        let html = r#"
            <table>
                <thead>
                    <tr><th>Name</th><th>Role</th></tr>
                </thead>
                <tbody>
                    <tr><td>Alice</td><td>Engineer</td></tr>
                    <tr><td>Bob</td><td>Designer</td></tr>
                </tbody>
            </table>
        "#;
        let base = Url::parse("https://example.com").unwrap();
        let md = html_to_markdown(html, &base);

        assert!(md.contains("| Name | Role |"));
        assert!(md.contains("| --- | --- |"));
        assert!(md.contains("| Alice | Engineer |"));
        assert!(md.contains("| Bob | Designer |"));
    }

    #[test]
    fn test_html_to_markdown_blockquote() {
        let html = r#"
            <blockquote>
                <p>Rust empowers everyone to build reliable and efficient software.</p>
            </blockquote>
        "#;
        let base = Url::parse("https://example.com").unwrap();
        let md = html_to_markdown(html, &base);

        assert!(md.contains("> Rust empowers everyone to build reliable and efficient software."));
    }
}
