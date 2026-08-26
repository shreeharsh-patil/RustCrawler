use ego_tree::NodeRef;
use scraper::node::Node;
use scraper::{Html, Node as ScraperNode};

const STRIPPED_TAGS: &[&str] = &[
    "script", "style", "noscript", "iframe", "canvas", "svg", "form", "input", "button", "select",
    "textarea", "option", "template", "head", "meta", "link", "dialog", "object", "embed",
    "applet",
];

const PRESERVED_TAGS: &[&str] = &[
    "html",
    "body",
    "main",
    "article",
    "section",
    "header",
    "footer",
    "nav",
    "aside",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "p",
    "div",
    "span",
    "blockquote",
    "pre",
    "code",
    "b",
    "strong",
    "i",
    "em",
    "u",
    "s",
    "del",
    "strike",
    "mark",
    "sub",
    "sup",
    "small",
    "a",
    "img",
    "picture",
    "figure",
    "figcaption",
    "ul",
    "ol",
    "li",
    "dl",
    "dt",
    "dd",
    "table",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "th",
    "td",
    "caption",
    "hr",
    "br",
];

/// Cleans raw HTML by stripping boilerplate, scripts, styles, forms, and tracking pixels
/// while preserving semantic markup and essential attributes.
pub fn clean_html(raw_html: &str) -> String {
    let parsed = Html::parse_fragment(raw_html);
    let mut output = String::with_capacity(raw_html.len());

    for child in parsed.tree.root().children() {
        clean_node(&child, &mut output, false);
    }

    collapse_excessive_whitespace(&output)
}

fn clean_node(node: &NodeRef<Node>, output: &mut String, inside_pre: bool) {
    match node.value() {
        ScraperNode::Text(text) => {
            if inside_pre {
                output.push_str(text);
            } else {
                let text_str: &str = text;
                let mut prev_space =
                    output.ends_with(' ') || output.ends_with('\n') || output.ends_with('>');
                for c in text_str.chars() {
                    if c.is_whitespace() {
                        if !prev_space {
                            output.push(' ');
                            prev_space = true;
                        }
                    } else {
                        output.push(c);
                        prev_space = false;
                    }
                }
            }
        }
        ScraperNode::Element(elem) => {
            let tag_name = elem.name().to_lowercase();

            // 1. Completely remove stripped tags
            if STRIPPED_TAGS.contains(&tag_name.as_str()) {
                return;
            }

            // 2. Check for hidden elements or tracking pixels
            if let Some(style) = elem.attr("style") {
                let style_lower = style.to_lowercase();
                if style_lower.contains("display:none")
                    || style_lower.contains("display: none")
                    || style_lower.contains("visibility:hidden")
                {
                    return;
                }
            }

            if elem.attr("hidden").is_some() || elem.attr("aria-hidden") == Some("true") {
                return;
            }

            // Check tracking pixel images (1x1 dimensions)
            if tag_name == "img" {
                let w = elem
                    .attr("width")
                    .and_then(|w| w.trim().parse::<u32>().ok());
                let h = elem
                    .attr("height")
                    .and_then(|h| h.trim().parse::<u32>().ok());
                if (w == Some(1) && h == Some(1)) || (w == Some(0) && h == Some(0)) {
                    return;
                }
            }

            // 3. Self-closing elements like <br> and <hr>
            if tag_name == "br" {
                output.push_str("<br>");
                return;
            }
            if tag_name == "hr" {
                output.push_str("<hr>");
                return;
            }

            let is_pre = tag_name == "pre" || inside_pre;
            let is_known = PRESERVED_TAGS.contains(&tag_name.as_str());

            if is_known {
                output.push('<');
                output.push_str(&tag_name);

                // Preserve useful attributes
                if let Some(href) = elem.attr("href") {
                    let escaped = html_escape(href);
                    output.push_str(&format!(" href=\"{escaped}\""));
                }
                if let Some(src) = elem.attr("src") {
                    let escaped = html_escape(src);
                    output.push_str(&format!(" src=\"{escaped}\""));
                }
                if let Some(srcset) = elem.attr("srcset") {
                    let escaped = html_escape(srcset);
                    output.push_str(&format!(" srcset=\"{escaped}\""));
                }
                if let Some(alt) = elem.attr("alt") {
                    let escaped = html_escape(alt);
                    output.push_str(&format!(" alt=\"{escaped}\""));
                }
                if let Some(title) = elem.attr("title") {
                    let escaped = html_escape(title);
                    output.push_str(&format!(" title=\"{escaped}\""));
                }
                if let Some(colspan) = elem.attr("colspan") {
                    let escaped = html_escape(colspan);
                    output.push_str(&format!(" colspan=\"{escaped}\""));
                }
                if let Some(rowspan) = elem.attr("rowspan") {
                    let escaped = html_escape(rowspan);
                    output.push_str(&format!(" rowspan=\"{escaped}\""));
                }
                if (tag_name == "code" || tag_name == "pre") && elem.attr("class").is_some() {
                    let class_val = elem.attr("class").unwrap();
                    let escaped = html_escape(class_val);
                    output.push_str(&format!(" class=\"{escaped}\""));
                }
                if tag_name.starts_with('h') && elem.attr("id").is_some() {
                    let id_val = elem.attr("id").unwrap();
                    let escaped = html_escape(id_val);
                    output.push_str(&format!(" id=\"{escaped}\""));
                }

                if tag_name == "img" {
                    output.push_str(" />");
                    return;
                }

                output.push('>');
            }

            // Recurse children
            for child in node.children() {
                clean_node(&child, output, is_pre);
            }

            if is_known && tag_name != "img" {
                output.push_str("</");
                output.push_str(&tag_name);
                output.push('>');
            }
        }
        _ => {}
    }
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn collapse_excessive_whitespace(html: &str) -> String {
    let mut res = String::with_capacity(html.len());
    let mut in_pre = false;

    let lines = html.lines();
    let mut consecutive_empty_lines = 0;

    for line in lines {
        let trimmed = line.trim();

        if trimmed.contains("<pre") {
            in_pre = true;
        }

        if in_pre {
            res.push_str(line);
            res.push('\n');
            if trimmed.contains("</pre>") {
                in_pre = false;
            }
            continue;
        }

        if trimmed.is_empty() {
            consecutive_empty_lines += 1;
            if consecutive_empty_lines <= 1 {
                res.push('\n');
            }
        } else {
            consecutive_empty_lines = 0;
            res.push_str(trimmed);
            res.push('\n');
        }
    }

    res.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_html_removes_scripts_and_styles() {
        let dirty = r#"
            <div>
                <script>alert('xss');</script>
                <style>body { color: red; }</style>
                <h1>Title</h1>
                <p>Paragraph with <script>bad()</script> content.</p>
                <form action="/login"><input type="text"><button>Submit</button></form>
                <img src="/track.png" width="1" height="1" />
                <img src="/photo.jpg" alt="Photo" />
            </div>
        "#;

        let cleaned = clean_html(dirty);

        assert!(!cleaned.contains("<script"));
        assert!(!cleaned.contains("<style"));
        assert!(!cleaned.contains("<form"));
        assert!(!cleaned.contains("<input"));
        assert!(!cleaned.contains("<button"));
        assert!(!cleaned.contains("track.png"));
        assert!(cleaned.contains("<h1>Title</h1>"));
        assert!(cleaned.contains("<p>Paragraph with content.</p>"));
        assert!(cleaned.contains("<img src=\"/photo.jpg\" alt=\"Photo\" />"));
    }
}
