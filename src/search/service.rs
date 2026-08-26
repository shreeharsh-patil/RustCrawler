use crate::config::Config;
use crate::crawl::dedupe::normalize_crawl_url;
use crate::discovery::models::MapRequest;
use crate::discovery::service::MapService;
use crate::error::CrawlerError;
use crate::models::{OutputFormat, ScrapeRequest};
use crate::search::local_index::LocalSearchIndex;
use crate::search::models::{
    SearchMode, SearchRequest, SearchResponse, SearchResult, SearchResultSource,
};
use crate::search::provider::SearchProviderRegistry;
use crate::service::ScraperService;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;
use url::Url;

pub struct SearchService {
    config: Arc<Config>,
    scraper: Arc<ScraperService>,
    map_service: Arc<MapService>,
    providers: Arc<SearchProviderRegistry>,
    local_index: Arc<LocalSearchIndex>,
}

impl SearchService {
    pub fn new(
        config: Arc<Config>,
        scraper: Arc<ScraperService>,
        map_service: Arc<MapService>,
        providers: Option<Arc<SearchProviderRegistry>>,
        local_index: Option<Arc<LocalSearchIndex>>,
    ) -> Self {
        let provider_registry =
            providers.unwrap_or_else(|| Arc::new(SearchProviderRegistry::new(&config)));
        let index = local_index.unwrap_or_else(|| Arc::new(LocalSearchIndex::new(10000)));

        Self {
            config,
            scraper,
            map_service,
            providers: provider_registry,
            local_index: index,
        }
    }

    pub fn local_index(&self) -> &Arc<LocalSearchIndex> {
        &self.local_index
    }

    pub fn providers(&self) -> &Arc<SearchProviderRegistry> {
        &self.providers
    }

    pub async fn search(&self, request: SearchRequest) -> Result<SearchResponse, CrawlerError> {
        let start_time = Instant::now();
        let limit = request.limit.clamp(1, self.config.max_search_results);

        let warnings = Vec::new();
        let mode = request.mode.unwrap_or(SearchMode::Web);

        // 1. Fetch raw search results according to search mode
        let mut raw_results = match mode {
            SearchMode::LocalIndex => self.local_index.search(&request.query, limit),
            SearchMode::Site => {
                if let Some(ref target_domain) = request.domain {
                    let site_url = if target_domain.starts_with("http://")
                        || target_domain.starts_with("https://")
                    {
                        target_domain.clone()
                    } else {
                        format!("https://{target_domain}")
                    };

                    let map_req = MapRequest {
                        url: site_url,
                        limit,
                        search: Some(request.query.clone()),
                        include_subdomains: false,
                        include_sitemap: true,
                        include_paths: Vec::new(),
                        exclude_paths: Vec::new(),
                        ignore_query_parameters: true,
                        render_dynamic_links: false,
                        max_depth: 2,
                    };

                    let map_resp = self.map_service.map(map_req).await?;
                    map_resp
                        .links
                        .into_iter()
                        .filter_map(|l| {
                            Url::parse(&l.url).ok().map(|u| SearchResult {
                                url: u,
                                title: l.title,
                                description: l.description,
                                score: l.score,
                                published_at: None,
                                source: SearchResultSource::SiteMap,
                                content: None,
                            })
                        })
                        .collect()
                } else {
                    let provider = self.providers.get(request.provider.as_deref())?;
                    provider.search(&request).await?
                }
            }
            SearchMode::Web => {
                let provider = self.providers.get(request.provider.as_deref())?;
                provider.search(&request).await?
            }
        };

        // 2. Normalization, Deduplication and Domain Filtering
        let mut seen_urls = HashSet::new();
        let mut deduplicated = Vec::new();

        for item in raw_results.drain(..) {
            let norm_str = normalize_crawl_url(&item.url, false, true);
            let norm_url = Url::parse(&norm_str).unwrap_or_else(|_| item.url.clone());

            // Domain filter check
            if let Some(ref req_domain) = request.domain {
                if let Some(host) = norm_url.host_str() {
                    if !host.contains(req_domain) {
                        continue;
                    }
                }
            }

            // Exclude domains check
            if let Some(host) = norm_url.host_str() {
                if request.exclude_domains.iter().any(|ex| host.contains(ex)) {
                    continue;
                }
            }

            if seen_urls.insert(norm_str) {
                deduplicated.push(SearchResult {
                    url: norm_url,
                    ..item
                });
            }
        }

        if deduplicated.len() > limit {
            deduplicated.truncate(limit);
        }

        // 3. Optional Bounded Parallel Scraping
        if request.scrape && !deduplicated.is_empty() {
            let max_scrapes = self
                .config
                .max_search_scrape_results
                .min(deduplicated.len());
            let max_concurrent = self.config.max_concurrent_search_scrapes.clamp(1, 20);
            let semaphore = Arc::new(Semaphore::new(max_concurrent));

            let mut scrape_tasks = Vec::new();
            for item in deduplicated.iter().take(max_scrapes) {
                let url_str = item.url.to_string();
                let sem = semaphore.clone();
                let scraper = self.scraper.clone();
                let formats_vec: Vec<OutputFormat> = if !request.formats.is_empty() {
                    request
                        .formats
                        .iter()
                        .filter_map(|f| OutputFormat::from_str(f).ok())
                        .collect()
                } else {
                    vec![OutputFormat::Markdown]
                };

                scrape_tasks.push(tokio::spawn(async move {
                    let _permit = sem.acquire().await.ok();
                    let mut scrape_req = ScrapeRequest::new(url_str);
                    scrape_req.formats = Some(formats_vec);
                    scraper.scrape(scrape_req).await.ok()
                }));
            }

            for (i, task) in scrape_tasks.into_iter().enumerate() {
                if let Ok(Some(scraped_doc)) = task.await {
                    deduplicated[i].content = Some(scraped_doc);
                }
            }
        }

        let total = deduplicated.len();
        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(SearchResponse {
            success: true,
            data: deduplicated,
            total,
            duration_ms,
            warnings,
        })
    }
}
