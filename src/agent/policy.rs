use url::Url;

use crate::agent::models::SourceType;
use crate::error::CrawlerError;
use crate::fetch::validation::validate_url_host_ssrf_preflight;

/// Security and policy engine enforcing read-only guarantees, URL protection, and source authority ranking
pub struct AgentPolicy;

impl AgentPolicy {
    /// Validates that a requested URL is permitted for agent operations
    pub fn validate_url(url_str: &str, allow_private: bool) -> Result<Url, CrawlerError> {
        let parsed = Url::parse(url_str).map_err(|e| {
            CrawlerError::InvalidUrl(format!("Failed to parse URL '{url_str}': {e}"))
        })?;

        // Enforce scheme
        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            return Err(CrawlerError::UnsupportedScheme(parsed.scheme().to_string()));
        }

        // Validate SSRF and private IP blocking
        validate_url_host_ssrf_preflight(&parsed, allow_private)?;

        Ok(parsed)
    }

    /// Classifies a source URL and metadata to compute authority ranking
    pub fn classify_source(url_str: &str, title: Option<&str>) -> SourceType {
        let Ok(parsed) = Url::parse(url_str) else {
            return SourceType::Unknown;
        };

        let host = parsed.host_str().unwrap_or("").to_lowercase();
        let path = parsed.path().to_lowercase();
        let _title_lower = title.map(|t| t.to_lowercase()).unwrap_or_default();

        // 1. Government
        if host.ends_with(".gov") || host.ends_with(".gov.uk") || host.ends_with(".mil") {
            return SourceType::Government;
        }

        // 2. Academic
        if host.ends_with(".edu")
            || host.ends_with(".ac.uk")
            || host.contains("arxiv.org")
            || host.contains("doi.org")
            || host.contains("biorxiv.org")
            || host.contains("semanticscholar.org")
            || host.contains("nature.com")
            || host.contains("ieee.org")
            || host.contains("acm.org")
        {
            return SourceType::Academic;
        }

        // 3. Repositories and package registries
        if host == "github.com"
            || host == "gitlab.com"
            || host == "crates.io"
            || host == "npmjs.com"
            || host == "pypi.org"
            || host == "pkg.go.dev"
            || host == "rubygems.org"
        {
            return SourceType::Repository;
        }

        // 4. Documentation
        if host.starts_with("docs.")
            || host.contains("readthedocs.io")
            || host.starts_with("developer.")
            || host.contains("gitbook.io")
            || path.starts_with("/doc")
            || path.starts_with("/documentation")
            || path.starts_with("/guide")
            || path.starts_with("/api")
            || path.starts_with("/reference")
        {
            return SourceType::Documentation;
        }

        // 5. News / Media
        if host.contains("reuters.com")
            || host.contains("bloomberg.com")
            || host.contains("bbc.com")
            || host.contains("techcrunch.com")
            || host.contains("nytimes.com")
            || host.contains("wsj.com")
            || host.contains("theverge.com")
            || host.contains("arstechnica.com")
            || host.contains("zdnet.com")
        {
            return SourceType::News;
        }

        // 6. Community / Forums
        if host.contains("reddit.com")
            || host.contains("news.ycombinator.com")
            || host.contains("stackoverflow.com")
            || host.contains("stackexchange.com")
            || host.contains("quora.com")
            || host.contains("medium.com")
            || host.contains("dev.to")
            || host.contains("discord.com")
        {
            return SourceType::Community;
        }

        SourceType::Primary
    }

    /// Sanitizes untrusted text to prevent prompt injection attacks when providing evidence to LLMs
    pub fn sanitize_observation(text: &str, max_chars: usize) -> String {
        let truncated = if text.chars().count() > max_chars {
            let mut s: String = text.chars().take(max_chars).collect();
            s.push_str("... [truncated]");
            s
        } else {
            text.to_string()
        };

        // Escape dangerous delimiter tags
        truncated
            .replace("</evidence_item>", "&lt;/evidence_item&gt;")
            .replace("</sources>", "&lt;/sources&gt;")
            .replace("</system_instruction>", "&lt;/system_instruction&gt;")
    }
}
