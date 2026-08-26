use crate::cache::http_cache::HttpCacheUtils;
use crate::cache::store::{CacheEntry, CacheStore, InMemoryCacheStore};
use crate::config::Config;
use crate::crawl::dedupe::normalize_crawl_url;
use crate::crawl::models::DiscoverySource;
use crate::crawl::robots::ParsedRobots;
use crate::discovery::lightweight::LightweightExtractor;
use crate::discovery::models::{DiscoveredLink, MapMetadata, MapRequest, MapResponse};
use crate::discovery::ranking::RelevanceRanker;
use crate::discovery::sitemap::{ParsedSitemapContent, StreamingSitemapParser};
use crate::error::CrawlerError;
use crate::fetch::HttpFetcher;
use crate::models::{ScrapeRequest, ScrapeWarning, WarningCode};
use crate::render::RenderMode;
use crate::service::ScraperService;
use crate::utils::patterns::matches_glob;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;
use tracing::debug;
use url::Url;

pub struct MapService {
    config: Arc<Config>,
    scraper: Arc<ScraperService>,
    fetcher: Arc<HttpFetcher>,
    cache: Arc<dyn CacheStore>,
}

impl MapService {
    pub fn new(
        config: Arc<Config>,
        scraper: Arc<ScraperService>,
        cache: Option<Arc<dyn CacheStore>>,
    ) -> Self {
        let fetcher = Arc::new(
            HttpFetcher::new(config.clone())
                .unwrap_or_else(|_| panic!("Failed to build HTTP fetcher for MapService")),
        );
        let cache_store: Arc<dyn CacheStore> = cache.unwrap_or_else(|| {
            Arc::new(InMemoryCacheStore::new(
                config.cache_enabled,
                config.cache_max_entries,
            ))
        });

        Self {
            config,
            scraper,
            fetcher,
            cache: cache_store,
        }
    }

    /// Primary entrypoint for discovering, mapping, and optionally ranking a website's URLs.
    pub async fn map(&self, request: MapRequest) -> Result<MapResponse, CrawlerError> {
        let start_time = Instant::now();
        let target_url =
            Url::parse(&request.url).map_err(|_| CrawlerError::InvalidUrl(request.url.clone()))?;

        if target_url.scheme() != "http" && target_url.scheme() != "https" {
            return Err(CrawlerError::UnsupportedScheme(
                target_url.scheme().to_string(),
            ));
        }

        let cache_key = HttpCacheUtils::compute_key(
            "map",
            &format!(
                "{}:{}",
                request.url,
                request.search.as_deref().unwrap_or("")
            ),
        );

        if let Ok(Some(cached)) = self.cache.get(&cache_key).await {
            if let Ok(cached_resp) = serde_json::from_slice::<MapResponse>(&cached.data) {
                debug!("Map cache hit for {}", request.url);
                return Ok(cached_resp);
            }
        }

        let mut discovered_links: Vec<DiscoveredLink> = Vec::new();
        let mut seen_urls: HashSet<String> = HashSet::new();
        let mut warnings: Vec<ScrapeWarning> = Vec::new();

        let base_host = target_url.host_str().unwrap_or_default().to_string();

        // 1. Robots.txt and Sitemap extraction
        let mut sitemap_candidates = Vec::new();
        if request.include_sitemap {
            let robots_url = target_url.join("/robots.txt").ok();
            if let Some(r_url) = robots_url {
                if let Ok(fetched_robots) = self.fetch_url_bytes(&r_url).await {
                    let robots_txt = String::from_utf8_lossy(&fetched_robots);
                    let parsed_robots = ParsedRobots::parse(&robots_txt);
                    for sm in &parsed_robots.sitemaps {
                        if let Ok(parsed_sm_url) = Url::parse(sm) {
                            sitemap_candidates.push(parsed_sm_url);
                        }
                    }
                }
            }

            // Probe standard sitemap locations
            if let Ok(standard_sitemap) = target_url.join("/sitemap.xml") {
                if !sitemap_candidates.contains(&standard_sitemap) {
                    sitemap_candidates.push(standard_sitemap);
                }
            }
            if let Ok(standard_index) = target_url.join("/sitemap_index.xml") {
                if !sitemap_candidates.contains(&standard_index) {
                    sitemap_candidates.push(standard_index);
                }
            }

            // 2. Parse sitemaps streaming
            self.crawl_sitemaps(
                &sitemap_candidates,
                &mut discovered_links,
                &mut seen_urls,
                &request,
                &base_host,
            )
            .await;
        }

        // 3. Homepage & Lightweight HTML Discovery
        let need_html_discovery = discovered_links.len() < request.limit;
        if need_html_discovery {
            self.crawl_html_discovery(
                &target_url,
                &mut discovered_links,
                &mut seen_urls,
                &request,
                &base_host,
                &mut warnings,
            )
            .await;
        }

        let total_discovered = discovered_links.len();

        // 4. Local Relevance Ranking if search query provided
        if let Some(ref query) = request.search {
            RelevanceRanker::rank_links(&mut discovered_links, query);
        }

        // Enforce limit
        if discovered_links.len() > request.limit {
            discovered_links.truncate(request.limit);
            warnings.push(ScrapeWarning::new(
                WarningCode::MapResultsTruncated,
                format!(
                    "Discovered links truncated to requested limit of {}",
                    request.limit
                ),
            ));
        }

        let returned = discovered_links.len();
        let duration_ms = start_time.elapsed().as_millis() as u64;

        let response = MapResponse {
            success: true,
            links: discovered_links,
            metadata: MapMetadata {
                discovered: total_discovered,
                returned,
                duration_ms,
            },
            warnings,
        };

        if self.config.map_cache_ttl_seconds > 0 {
            if let Ok(json_bytes) = serde_json::to_vec(&response) {
                let cache_entry = CacheEntry::new(
                    json_bytes,
                    Some("application/json".to_string()),
                    200,
                    None,
                    None,
                    None,
                    self.config.map_cache_ttl_seconds,
                );
                let _ = self.cache.put(cache_key, cache_entry).await;
            }
        }

        Ok(response)
    }

    async fn crawl_sitemaps(
        &self,
        initial_sitemaps: &[Url],
        discovered_links: &mut Vec<DiscoveredLink>,
        seen_urls: &mut HashSet<String>,
        request: &MapRequest,
        base_host: &str,
    ) {
        let mut sitemap_queue: VecDeque<(Url, usize)> = VecDeque::new();
        let mut visited_sitemaps: HashSet<String> = HashSet::new();

        for sm in initial_sitemaps {
            sitemap_queue.push_back((sm.clone(), 0));
        }

        let max_files = self.config.max_sitemap_files;
        let max_urls = self.config.max_sitemap_urls;
        let max_depth = self.config.max_sitemap_depth;
        let max_file_bytes = self.config.max_sitemap_file_size_bytes();

        while let Some((sitemap_url, depth)) = sitemap_queue.pop_front() {
            if visited_sitemaps.len() >= max_files || depth > max_depth {
                break;
            }

            let sitemap_key = sitemap_url.to_string();
            if !visited_sitemaps.insert(sitemap_key) {
                continue;
            }

            if let Ok(bytes) = self.fetch_url_bytes(&sitemap_url).await {
                if let Ok(parsed) =
                    StreamingSitemapParser::parse_sitemap_bytes(&bytes, max_urls, max_file_bytes)
                {
                    match parsed {
                        ParsedSitemapContent::Urls(entries) => {
                            for entry in entries {
                                let norm_str = normalize_crawl_url(
                                    &entry.loc,
                                    request.ignore_query_parameters,
                                    true,
                                );
                                let norm_url =
                                    Url::parse(&norm_str).unwrap_or_else(|_| entry.loc.clone());

                                if self.is_url_allowed(&norm_url, request, base_host)
                                    && seen_urls.insert(norm_str.clone())
                                {
                                    discovered_links.push(DiscoveredLink {
                                        url: norm_str,
                                        title: None,
                                        description: None,
                                        source: DiscoverySource::Sitemap,
                                        score: None,
                                    });

                                    if discovered_links.len() >= request.limit * 2 {
                                        return;
                                    }
                                }
                            }
                        }
                        ParsedSitemapContent::SitemapIndex(child_sitemaps) => {
                            for child in child_sitemaps {
                                if !visited_sitemaps.contains(child.as_str())
                                    && sitemap_queue.len() < max_files
                                {
                                    sitemap_queue.push_back((child, depth + 1));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    async fn crawl_html_discovery(
        &self,
        seed_url: &Url,
        discovered_links: &mut Vec<DiscoveredLink>,
        seen_urls: &mut HashSet<String>,
        request: &MapRequest,
        base_host: &str,
        warnings: &mut Vec<ScrapeWarning>,
    ) {
        let mut queue: VecDeque<(Url, u32)> = VecDeque::new();
        queue.push_back((seed_url.clone(), 0));

        let norm_seed_str = normalize_crawl_url(seed_url, request.ignore_query_parameters, true);
        let _norm_seed = Url::parse(&norm_seed_str).unwrap_or_else(|_| seed_url.clone());
        seen_urls.insert(norm_seed_str.clone());

        let max_concurrent = self.config.max_concurrent_discovery_fetches.clamp(1, 100);
        let semaphore = Arc::new(Semaphore::new(max_concurrent));

        while !queue.is_empty() && discovered_links.len() < request.limit * 2 {
            let mut batch = Vec::new();
            while let Some((url, depth)) = queue.pop_front() {
                batch.push((url, depth));
                if batch.len() >= max_concurrent {
                    break;
                }
            }

            let mut tasks = Vec::new();
            for (url, depth) in batch {
                let sem = semaphore.clone();
                let fetcher = self.fetcher.clone();
                let scraper = self.scraper.clone();
                let render_dynamic = request.render_dynamic_links;
                let max_links = self.config.max_page_links_discovered;
                let max_anchor = self.config.max_anchor_text_length;

                tasks.push(tokio::spawn(async move {
                    let _permit = sem.acquire().await.ok();

                    // If dynamic links requested, use browser
                    if render_dynamic {
                        let mut scrape_req = ScrapeRequest::new(url.to_string());
                        scrape_req.render_mode = Some(RenderMode::Browser);
                        if let Ok(res) = scraper.scrape(scrape_req).await {
                            let html_content = res.html.unwrap_or_default();
                            let summary = LightweightExtractor::extract(
                                &html_content,
                                &url,
                                max_links,
                                max_anchor,
                            );
                            return Some((url, depth, summary));
                        }
                    }

                    if let Ok(fetched) = fetcher.fetch(url.as_str()).await {
                        let summary = LightweightExtractor::extract(
                            &fetched.html,
                            &url,
                            max_links,
                            max_anchor,
                        );
                        return Some((url, depth, summary));
                    }

                    None
                }));
            }

            for task in tasks {
                if let Ok(Some((page_url, depth, summary))) = task.await {
                    let page_norm =
                        normalize_crawl_url(&page_url, request.ignore_query_parameters, true);

                    // Update or insert page node
                    if let Some(existing) = discovered_links.iter_mut().find(|l| l.url == page_norm)
                    {
                        if existing.title.is_none() {
                            existing.title = summary.title.clone();
                        }
                        if existing.description.is_none() {
                            existing.description = summary.description.clone();
                        }
                    } else if self.is_url_allowed(&page_url, request, base_host) {
                        discovered_links.push(DiscoveredLink {
                            url: page_norm,
                            title: summary.title.clone(),
                            description: summary.description.clone(),
                            source: if depth == 0 {
                                DiscoverySource::Seed
                            } else {
                                DiscoverySource::HtmlLink
                            },
                            score: None,
                        });
                    }

                    // Enqueue discovered outbound links if depth within max_depth
                    if depth < request.max_depth {
                        for (out_url, _anchor) in summary.links {
                            let out_norm_str = normalize_crawl_url(
                                &out_url,
                                request.ignore_query_parameters,
                                true,
                            );
                            let out_norm =
                                Url::parse(&out_norm_str).unwrap_or_else(|_| out_url.clone());

                            if self.is_url_allowed(&out_norm, request, base_host)
                                && seen_urls.insert(out_norm_str.clone())
                            {
                                queue.push_back((out_norm, depth + 1));
                                if discovered_links.len() < request.limit * 2 {
                                    discovered_links.push(DiscoveredLink {
                                        url: out_norm_str,
                                        title: None,
                                        description: None,
                                        source: DiscoverySource::HtmlLink,
                                        score: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        if !queue.is_empty() {
            warnings.push(ScrapeWarning::new(
                WarningCode::DiscoveryPageLimitReached,
                "Discovery queue had remaining links when discovery limit was met",
            ));
        }
    }

    fn is_url_allowed(&self, url: &Url, request: &MapRequest, base_host: &str) -> bool {
        let host = match url.host_str() {
            Some(h) => h,
            None => return false,
        };

        if request.include_subdomains {
            if !host.ends_with(base_host) {
                return false;
            }
        } else if host != base_host {
            return false;
        }

        let path = url.path();

        if !request.include_paths.is_empty() {
            let matched = request.include_paths.iter().any(|p| matches_glob(p, path));
            if !matched {
                return false;
            }
        }

        if !request.exclude_paths.is_empty() {
            let matched = request.exclude_paths.iter().any(|p| matches_glob(p, path));
            if matched {
                return false;
            }
        }

        true
    }

    async fn fetch_url_bytes(&self, url: &Url) -> Result<Vec<u8>, CrawlerError> {
        let fetched = self.fetcher.fetch(url.as_str()).await?;
        Ok(fetched.html.into_bytes())
    }
}
