use crate::models::{ScrapeWarning, WarningCode};
use scraper::{ElementRef, Html, Selector};
use std::sync::LazyLock;

static ARTICLE_SEL: LazyLock<Selector> = LazyLock::new(|| Selector::parse("article").unwrap());
static MAIN_SEL: LazyLock<Selector> = LazyLock::new(|| Selector::parse("main").unwrap());
static CANDIDATE_SEL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("main, article, div, section").unwrap());
static P_SEL: LazyLock<Selector> = LazyLock::new(|| Selector::parse("p").unwrap());
static A_SEL: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a").unwrap());
static H_SEL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("h1, h2, h3, h4, h5, h6").unwrap());

const POSITIVE_TERMS: &[&str] = &[
    "article", "content", "post", "entry", "story", "main", "body", "text", "blog", "detail",
    "page", "readable",
];

const NEGATIVE_TERMS: &[&str] = &[
    "nav",
    "navigation",
    "menu",
    "footer",
    "header",
    "sidebar",
    "aside",
    "advert",
    "advertisement",
    "ads",
    "ad-",
    "cookie",
    "banner",
    "popup",
    "modal",
    "related",
    "comments",
    "comment",
    "social",
    "share",
    "widget",
    "promo",
    "breadcrumb",
    "disclaimer",
    "signup",
    "newsletter",
];

/// Extracts the main content HTML string from a full document HTML.
pub fn extract_main_content(html: &Html, warnings: &mut Vec<ScrapeWarning>) -> String {
    // 1. Check for prominent <article> tags first
    let articles: Vec<ElementRef> = html.select(&ARTICLE_SEL).collect();
    if articles.len() == 1 {
        let text = get_text_length(articles[0]);
        if text >= 150 {
            return articles[0].html();
        }
    } else if articles.len() > 1 {
        // Pick the best article
        let mut best_article = None;
        let mut best_score = 0.0;
        for art in &articles {
            let score = score_element(*art);
            if score > best_score {
                best_score = score;
                best_article = Some(*art);
            }
        }
        if let Some(art) = best_article {
            if best_score > 30.0 {
                return art.html();
            }
        }
    }

    // 2. Check for <main> tag
    if let Some(main_elem) = html.select(&MAIN_SEL).next() {
        let text = get_text_length(main_elem);
        if text >= 100 {
            return main_elem.html();
        }
    }

    // 3. Heuristic candidate scoring
    let mut best_elem: Option<ElementRef> = None;
    let mut best_score = f64::MIN;

    for elem in html.select(&CANDIDATE_SEL) {
        let score = score_element(elem);
        if score > best_score {
            best_score = score;
            best_elem = Some(elem);
        }
    }

    if let Some(elem) = best_elem {
        if best_score > 15.0 {
            return elem.html();
        }
    }

    // 4. Fallback to <body> or entire document
    static BODY_SEL: LazyLock<Selector> = LazyLock::new(|| Selector::parse("body").unwrap());
    if let Some(body) = html.select(&BODY_SEL).next() {
        warnings.push(ScrapeWarning::new(
            WarningCode::MissingMainContent,
            "Could not identify a distinct main-content container; used body fallback",
        ));
        return body.html();
    }

    warnings.push(ScrapeWarning::new(
        WarningCode::MissingMainContent,
        "Could not identify main content; returned full document",
    ));
    html.html()
}

/// Computes a readability heuristic score for a candidate container element.
fn score_element(elem: ElementRef) -> f64 {
    let mut score = 0.0;
    let tag = elem.value().name();

    // Semantic tag bonus
    match tag {
        "article" => score += 35.0,
        "main" => score += 30.0,
        "section" => score += 15.0,
        "div" => score += 5.0,
        _ => {}
    }

    // Class and ID scoring
    let class_str = elem.value().attr("class").unwrap_or("").to_lowercase();
    let id_str = elem.value().attr("id").unwrap_or("").to_lowercase();
    let combined_attr = format!("{class_str} {id_str}");

    for &pos in POSITIVE_TERMS {
        if combined_attr.contains(pos) {
            score += 25.0;
        }
    }

    for &neg in NEGATIVE_TERMS {
        if combined_attr.contains(neg) {
            score -= 30.0;
        }
    }

    // Text & paragraph density
    let text_len = get_text_length(elem);
    score += (text_len as f64) / 80.0;

    let p_count = elem.select(&P_SEL).count();
    score += (p_count as f64) * 12.0;

    let h_count = elem.select(&H_SEL).count();
    score += (h_count as f64) * 6.0;

    // Link density penalty
    let mut link_text_len = 0;
    for a in elem.select(&A_SEL) {
        link_text_len += get_text_length(a);
    }

    let link_density = if text_len > 0 {
        link_text_len as f64 / text_len as f64
    } else {
        1.0
    };

    if link_density > 0.5 {
        score *= 1.0 - link_density;
    }

    score
}

fn get_text_length(elem: ElementRef) -> usize {
    elem.text().map(|t| t.trim().len()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_main_content_with_article() {
        let html_str = r#"
            <html>
            <body>
                <header><nav><a href="/">Home</a><a href="/about">About</a></nav></header>
                <div class="sidebar"><p>Ad sidebar</p></div>
                <article class="post-content">
                    <h1>Main Blog Post</h1>
                    <p>This is the first paragraph of the main article content.</p>
                    <p>This is the second paragraph with more extensive details about Rust programming.</p>
                </article>
                <footer><p>Copyright 2026</p></footer>
            </body>
            </html>
        "#;

        let parsed = Html::parse_document(html_str);
        let mut warnings = Vec::new();
        let main = extract_main_content(&parsed, &mut warnings);

        assert!(main.contains("Main Blog Post"));
        assert!(main.contains("This is the first paragraph"));
        assert!(!main.contains("Ad sidebar"));
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_extract_main_content_scoring_fallback() {
        let html_str = r#"
            <html>
            <body>
                <div class="menu-header">Menu</div>
                <div id="content" class="story-body">
                    <h2>Scored Story</h2>
                    <p>A long narrative with multiple sentences that provide high value information to the reader.</p>
                    <p>Another paragraph expanding on the story with more substantive content.</p>
                </div>
                <div class="ads-banner">Buy now!</div>
            </body>
            </html>
        "#;

        let parsed = Html::parse_document(html_str);
        let mut warnings = Vec::new();
        let main = extract_main_content(&parsed, &mut warnings);

        assert!(main.contains("Scored Story"));
        assert!(!main.contains("Buy now!"));
    }
}
