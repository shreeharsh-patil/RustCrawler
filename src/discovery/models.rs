use crate::crawl::models::DiscoverySource;
use crate::models::ScrapeWarning;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchPurpose {
    #[default]
    FullScrape,
    Discovery,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlNode {
    pub url: Url,
    pub title: Option<String>,
    pub description: Option<String>,
    pub depth: Option<u32>,
    pub source: DiscoverySource,
    pub parent_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveredLink {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub source: DiscoverySource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapMetadata {
    pub discovered: usize,
    pub returned: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapRequest {
    pub url: String,
    #[serde(default = "default_map_limit")]
    pub limit: usize,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub include_subdomains: bool,
    #[serde(default = "default_true")]
    pub include_sitemap: bool,
    #[serde(default)]
    pub include_paths: Vec<String>,
    #[serde(default)]
    pub exclude_paths: Vec<String>,
    #[serde(default)]
    pub ignore_query_parameters: bool,
    #[serde(default)]
    pub render_dynamic_links: bool,
    #[serde(default = "default_map_max_depth")]
    pub max_depth: u32,
}

fn default_map_limit() -> usize {
    5000
}

fn default_true() -> bool {
    true
}

fn default_map_max_depth() -> u32 {
    3
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapResponse {
    pub success: bool,
    pub links: Vec<DiscoveredLink>,
    pub metadata: MapMetadata,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub warnings: Vec<ScrapeWarning>,
}
