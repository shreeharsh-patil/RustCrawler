use crate::fetch::HttpFetcher;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};
use url::Url;

#[derive(Debug, Clone, Default)]
pub struct RobotsRules {
    pub allows: Vec<String>,
    pub disallows: Vec<String>,
    pub crawl_delay: Option<Duration>,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedRobots {
    pub user_agent_rules: Vec<(String, RobotsRules)>,
    pub default_rules: RobotsRules,
    pub sitemaps: Vec<String>,
}

impl ParsedRobots {
    /// Parses robots.txt content into structured rules.
    pub fn parse(content: &str) -> Self {
        let mut parsed = Self::default();
        let mut current_agents: Vec<String> = Vec::new();
        let mut current_rules = RobotsRules::default();

        for line in content.lines() {
            let clean_line = line.split('#').next().unwrap_or("").trim();
            if clean_line.is_empty() {
                continue;
            }

            if let Some((directive, value)) = clean_line.split_once(':') {
                let directive = directive.trim().to_ascii_lowercase();
                let value = value.trim();

                match directive.as_str() {
                    "user-agent" => {
                        // If we had accumulated rules for previous agents, store them
                        if !current_rules.allows.is_empty()
                            || !current_rules.disallows.is_empty()
                            || current_rules.crawl_delay.is_some()
                        {
                            for agent in &current_agents {
                                if agent == "*" {
                                    parsed.default_rules = current_rules.clone();
                                } else {
                                    parsed
                                        .user_agent_rules
                                        .push((agent.clone(), current_rules.clone()));
                                }
                            }
                            current_rules = RobotsRules::default();
                            current_agents.clear();
                        }
                        current_agents.push(value.to_ascii_lowercase());
                    }
                    "allow" => {
                        if !value.is_empty() {
                            current_rules.allows.push(value.to_string());
                        }
                    }
                    "disallow" => {
                        if !value.is_empty() {
                            current_rules.disallows.push(value.to_string());
                        }
                    }
                    "crawl-delay" => {
                        if let Ok(secs) = value.parse::<f64>() {
                            if secs > 0.0 {
                                current_rules.crawl_delay =
                                    Some(Duration::from_millis((secs * 1000.0) as u64));
                            }
                        }
                    }
                    "sitemap" => {
                        if let Ok(sitemap_url) = Url::parse(value) {
                            if sitemap_url.scheme() == "http" || sitemap_url.scheme() == "https" {
                                parsed.sitemaps.push(sitemap_url.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Flush last block
        for agent in &current_agents {
            if agent == "*" {
                parsed.default_rules = current_rules.clone();
            } else {
                parsed
                    .user_agent_rules
                    .push((agent.clone(), current_rules.clone()));
            }
        }

        parsed
    }

    /// Finds matching rules for a specific User-Agent string.
    pub fn get_rules_for_agent(&self, user_agent: &str) -> &RobotsRules {
        let ua_lower = user_agent.to_ascii_lowercase();
        for (agent, rules) in &self.user_agent_rules {
            if ua_lower.contains(agent) {
                return rules;
            }
        }
        &self.default_rules
    }

    /// Checks if a path is allowed according to robots.txt rules for the given User-Agent.
    pub fn is_path_allowed(&self, path: &str, user_agent: &str) -> bool {
        let rules = self.get_rules_for_agent(user_agent);

        let mut longest_allow = 0;
        for allow in &rules.allows {
            if path.starts_with(allow) {
                longest_allow = longest_allow.max(allow.len());
            }
        }

        let mut longest_disallow = 0;
        for disallow in &rules.disallows {
            if path.starts_with(disallow) {
                longest_disallow = longest_disallow.max(disallow.len());
            }
        }

        // If disallow matches and is strictly longer than allow match, block
        if longest_disallow > 0 && longest_disallow >= longest_allow {
            return false;
        }

        true
    }

    pub fn crawl_delay(&self, user_agent: &str) -> Option<Duration> {
        self.get_rules_for_agent(user_agent).crawl_delay
    }
}

/// Thread-safe host-level robots.txt manager and cache.
pub struct RobotsManager {
    cache: DashMap<String, Arc<ParsedRobots>>,
}

impl Default for RobotsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RobotsManager {
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }

    /// Retrieves or fetches and parses robots.txt for the given target URL's host.
    pub async fn get_or_fetch(&self, url: &Url, fetcher: &HttpFetcher) -> Arc<ParsedRobots> {
        let host = match url.host_str() {
            Some(h) => h.to_lowercase(),
            None => return Arc::new(ParsedRobots::default()),
        };

        let port_part = match url.port() {
            Some(p)
                if (p != 80 && url.scheme() == "http") || (p != 443 && url.scheme() == "https") =>
            {
                format!(":{p}")
            }
            _ => String::new(),
        };

        let host_key = format!("{}://{}{}", url.scheme(), host, port_part);

        if let Some(existing) = self.cache.get(&host_key) {
            return existing.value().clone();
        }

        let robots_url_str = format!("{host_key}/robots.txt");
        let parsed = match Url::parse(&robots_url_str) {
            Ok(robots_url) => {
                debug!("Fetching robots.txt from {}", robots_url);
                match fetcher.fetch(robots_url.as_str()).await {
                    Ok(doc) => {
                        if (200..300).contains(&doc.status) {
                            ParsedRobots::parse(&doc.html)
                        } else {
                            debug!("robots.txt returned status {}", doc.status);
                            ParsedRobots::default()
                        }
                    }
                    Err(e) => {
                        warn!("Failed to fetch robots.txt for {}: {}", host_key, e);
                        ParsedRobots::default()
                    }
                }
            }
            Err(_) => ParsedRobots::default(),
        };

        let arc_parsed = Arc::new(parsed);
        self.cache.insert(host_key, arc_parsed.clone());
        arc_parsed
    }

    pub fn insert_cached(&self, host_key: String, robots: ParsedRobots) {
        self.cache.insert(host_key, Arc::new(robots));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_robots_txt() {
        let txt = r#"
# Sample robots.txt
User-agent: Googlebot
Disallow: /private/
Allow: /private/public/

User-agent: *
Disallow: /admin/
Disallow: /api/
Allow: /api/docs/
Crawl-delay: 2.5

Sitemap: https://example.com/sitemap.xml
Sitemap: https://example.com/sitemap2.xml
"#;

        let parsed = ParsedRobots::parse(txt);

        assert_eq!(parsed.sitemaps.len(), 2);
        assert_eq!(parsed.sitemaps[0], "https://example.com/sitemap.xml");

        // Googlebot rules
        assert!(parsed.is_path_allowed("/private/public/page", "Googlebot"));
        assert!(!parsed.is_path_allowed("/private/secret", "Googlebot"));
        assert!(parsed.is_path_allowed("/admin/users", "Googlebot")); // default rule not applied because googlebot has dedicated section

        // Default rules
        assert!(!parsed.is_path_allowed("/admin/users", "RustCrawler/0.1"));
        assert!(!parsed.is_path_allowed("/api/users", "RustCrawler/0.1"));
        assert!(parsed.is_path_allowed("/api/docs/start", "RustCrawler/0.1"));
        assert!(parsed.is_path_allowed("/home", "RustCrawler/0.1"));

        assert_eq!(
            parsed.crawl_delay("RustCrawler/0.1"),
            Some(Duration::from_millis(2500))
        );
    }
}
