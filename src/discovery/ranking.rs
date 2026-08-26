use crate::discovery::models::DiscoveredLink;

pub struct RelevanceRanker;

impl RelevanceRanker {
    /// Ranks discovered links according to relevance to a search query string.
    pub fn rank_links(links: &mut [DiscoveredLink], query: &str) {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() {
            return;
        }

        for link in links.iter_mut() {
            let score = score_link(link, &query_tokens);
            link.score = Some(score);
        }

        links.sort_by(|a, b| {
            b.score
                .unwrap_or(0.0)
                .partial_cmp(&a.score.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

pub fn score_link(link: &DiscoveredLink, query_tokens: &[String]) -> f32 {
    let mut total_score = 0.0f32;

    let title_tokens = link.title.as_deref().map(tokenize).unwrap_or_default();
    let desc_tokens = link
        .description
        .as_deref()
        .map(tokenize)
        .unwrap_or_default();
    let url_tokens = tokenize_url_path(&link.url);

    // Weights: title (4.0) > url_slug (2.5) > description (1.0)
    for q_token in query_tokens {
        // 1. Exact match on title
        if title_tokens.iter().any(|t| t == q_token) {
            total_score += 4.0;
        } else if title_tokens
            .iter()
            .any(|t| t.contains(q_token) || q_token.contains(t))
        {
            total_score += 2.0;
        }

        // 2. URL slug / path segment match
        if url_tokens.iter().any(|t| t == q_token) {
            total_score += 2.5;
        } else if url_tokens.iter().any(|t| t.contains(q_token)) {
            total_score += 1.2;
        }

        // 3. Description match
        if desc_tokens.iter().any(|t| t == q_token) {
            total_score += 1.0;
        } else if desc_tokens.iter().any(|t| t.contains(q_token)) {
            total_score += 0.5;
        }
    }

    // Normalize to 0.0 .. 0.99
    let max_possible = query_tokens.len() as f32 * 7.5;
    let normalized = (total_score / max_possible).clamp(0.01, 0.99);
    (normalized * 100.0).round() / 100.0
}

pub fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .map(|w| w.trim().to_lowercase())
        .filter(|w| w.len() >= 2 && !is_stopword(w))
        .collect()
}

pub fn tokenize_url_path(url_str: &str) -> Vec<String> {
    if let Ok(parsed) = url::Url::parse(url_str) {
        let path = parsed.path();
        path.split(['/', '-', '_', '.'])
            .map(|s| s.trim().to_lowercase())
            .filter(|s| s.len() >= 2 && !is_stopword(s) && !is_file_ext(s))
            .collect()
    } else {
        Vec::new()
    }
}

fn is_stopword(word: &str) -> bool {
    matches!(
        word,
        "the"
            | "to"
            | "in"
            | "of"
            | "on"
            | "at"
            | "by"
            | "an"
            | "as"
            | "is"
            | "it"
            | "or"
            | "and"
            | "for"
            | "that"
            | "this"
            | "with"
            | "from"
            | "are"
            | "was"
            | "com"
            | "org"
            | "net"
            | "html"
            | "php"
            | "http"
            | "https"
            | "www"
    )
}

fn is_file_ext(word: &str) -> bool {
    matches!(
        word,
        "html" | "htm" | "php" | "asp" | "aspx" | "jsp" | "pdf" | "json"
    )
}
