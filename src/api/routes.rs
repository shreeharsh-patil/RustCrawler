use crate::agent::AgentService;
use crate::api::agent_routes::{
    cancel_agent_job, get_agent_job_status, pause_agent_job, resume_agent_job, stream_agent_events,
    submit_agent_job,
};
use crate::api::crawl_routes::{delete_crawl_handler, get_crawl_handler, post_crawl_handler};
use crate::api::discovery_routes::{
    get_crawl_changes_handler, post_incremental_crawl_handler, post_map_handler,
    post_search_handler,
};
use crate::api::distributed_routes::{
    cancel_job_handler, get_job_events_sse_handler, get_job_handler, get_job_results_handler,
    pause_job_handler, readiness_handler, resume_job_handler,
};
use crate::api::extraction_routes::{
    delete_extract_job_handler, get_extract_job_handler, post_extract_batch_handler,
    post_extract_handler,
};
use crate::api::knowledge_routes::{
    answer_handler, create_or_index_handler, delete_index_handler, get_index_stats_handler,
    index_crawl_handler, rebuild_index_handler, retrieve_handler, search_hybrid_handler,
    search_semantic_handler,
};
use crate::api::models::HealthResponse;
use crate::config::Config;
use crate::crawl::CrawlerService;
use crate::discovery::service::MapService;
use crate::distributed::coordinator::frontier::DistributedFrontier;
use crate::distributed::coordinator::scheduler::JobScheduler;
use crate::distributed::queue::{MemoryTaskQueue, TaskQueue};
use crate::distributed::results::{LocalResultStore, ResultStore};
use crate::distributed::store::{MemoryMetadataStore, MetadataStore};
use crate::error::CrawlerError;
use crate::extraction::service::ExtractionService;
use crate::knowledge::KnowledgeEngine;
use crate::models::{ScrapeRequest, ScrapeResponse};
use crate::search::service::SearchService;
use crate::service::ScraperService;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub scraper: Arc<ScraperService>,
    pub crawler: Arc<CrawlerService>,
    pub extraction: Arc<ExtractionService>,
    pub map_service: Arc<MapService>,
    pub search_service: Arc<SearchService>,
    pub metadata_store: Arc<dyn MetadataStore>,
    pub task_queue: Arc<dyn TaskQueue>,
    pub result_store: Arc<dyn ResultStore>,
    pub scheduler: Arc<JobScheduler>,
    pub knowledge_engine: Arc<KnowledgeEngine>,
    pub agent_service: Option<Arc<AgentService>>,
}

pub fn router(service: ScraperService) -> Router {
    let config = service.config().clone();
    let scraper = Arc::new(service);
    let crawler = Arc::new(CrawlerService::new(scraper.clone()));
    let extraction = Arc::new(ExtractionService::new(
        config.as_ref().clone(),
        scraper.clone(),
        crawler.clone(),
    ));
    let map_service = Arc::new(MapService::new(config.clone(), scraper.clone(), None));
    let search_service = Arc::new(SearchService::new(
        config.clone(),
        scraper.clone(),
        map_service.clone(),
        None,
        None,
    ));

    let metadata_store = Arc::new(MemoryMetadataStore::new());
    let task_queue = Arc::new(MemoryTaskQueue::default());
    let result_store = Arc::new(LocalResultStore::default());
    let frontier = Arc::new(DistributedFrontier::new());
    let scheduler = Arc::new(JobScheduler::new(
        metadata_store.clone(),
        task_queue.clone(),
        frontier,
    ));
    let knowledge_engine = Arc::new(KnowledgeEngine::new(config.clone(), None));

    let agent_service = Arc::new(AgentService::new(
        config.clone(),
        Some(search_service.clone()),
        Some(map_service.clone()),
        scraper.clone(),
        crawler.clone(),
        extraction.clone(),
        Some(knowledge_engine.clone()),
        None,
        None,
    ));

    app_router(
        config,
        scraper,
        crawler,
        extraction,
        map_service,
        search_service,
        metadata_store,
        task_queue,
        result_store,
        scheduler,
        knowledge_engine,
        Some(agent_service),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn app_router(
    config: Arc<Config>,
    scraper: Arc<ScraperService>,
    crawler: Arc<CrawlerService>,
    extraction: Arc<ExtractionService>,
    map_service: Arc<MapService>,
    search_service: Arc<SearchService>,
    metadata_store: Arc<dyn MetadataStore>,
    task_queue: Arc<dyn TaskQueue>,
    result_store: Arc<dyn ResultStore>,
    scheduler: Arc<JobScheduler>,
    knowledge_engine: Arc<KnowledgeEngine>,
    agent_service: Option<Arc<AgentService>>,
) -> Router {
    let state = AppState {
        config,
        scraper,
        crawler,
        extraction,
        map_service,
        search_service,
        metadata_store,
        task_queue,
        result_store,
        scheduler,
        knowledge_engine,
        agent_service,
    };

    Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(readiness_handler))
        .route("/v1/scrape", post(scrape_handler))
        .route("/v1/crawl", post(post_crawl_wrapper))
        .route(
            "/v1/crawl/:job_id",
            get(get_crawl_wrapper).delete(delete_crawl_wrapper),
        )
        .route(
            "/v1/crawl/incremental",
            post(post_incremental_crawl_handler),
        )
        .route("/v1/crawl/:job_id/changes", get(get_crawl_changes_handler))
        .route("/v1/map", post(post_map_handler))
        .route("/v1/search", post(post_search_handler))
        .route("/v1/extract", post(post_extract_wrapper))
        .route("/v1/extract/batch", post(post_extract_batch_wrapper))
        .route(
            "/v1/extract/:job_id",
            get(get_extract_job_wrapper).delete(delete_extract_job_wrapper),
        )
        .route(
            "/v1/jobs/:id",
            get(get_job_handler).delete(cancel_job_handler),
        )
        .route("/v1/jobs/:id/pause", post(pause_job_handler))
        .route("/v1/jobs/:id/resume", post(resume_job_handler))
        .route("/v1/jobs/:id/results", get(get_job_results_handler))
        .route("/v1/jobs/:id/events", get(get_job_events_sse_handler))
        .route("/v1/index", post(create_or_index_handler))
        .route("/v1/index/crawl", post(index_crawl_handler))
        .route("/v1/search/semantic", post(search_semantic_handler))
        .route("/v1/search/hybrid", post(search_hybrid_handler))
        .route("/v1/retrieve", post(retrieve_handler))
        .route("/v1/answer", post(answer_handler))
        .route(
            "/v1/index/:index_id",
            get(get_index_stats_handler).delete(delete_index_handler),
        )
        .route("/v1/index/:index_id/rebuild", post(rebuild_index_handler))
        .route("/v1/agent", post(submit_agent_job))
        .route(
            "/v1/agent/:job_id",
            get(get_agent_job_status).delete(cancel_agent_job),
        )
        .route("/v1/agent/:job_id/events", get(stream_agent_events))
        .route("/v1/agent/:job_id/pause", post(pause_agent_job))
        .route("/v1/agent/:job_id/resume", post(resume_agent_job))
        .with_state(state)
}

async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let bm = state.scraper.browser_manager();
    let browser_status = if !bm.is_enabled() {
        "disabled".to_string()
    } else if bm.is_healthy().await {
        "ready".to_string()
    } else {
        "idle_ready".to_string()
    };

    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok",
            version: env!("CARGO_PKG_VERSION"),
            http_scraper: "ok",
            document_parsers: "ready",
            extraction_engine: "ready",
            discovery_engine: "ready",
            search_engine: "ready",
            cache_store: "ready",
            browser: crate::api::models::BrowserHealthInfo {
                enabled: bm.is_enabled(),
                status: browser_status,
                active_pages: bm.active_pages(),
                max_pages: bm.max_pages(),
            },
        }),
    )
}

async fn scrape_handler(
    State(state): State<AppState>,
    Json(request): Json<ScrapeRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    let result = state.scraper.scrape(request).await?;
    Ok((StatusCode::OK, Json(ScrapeResponse::success(result))))
}

async fn post_crawl_wrapper(
    State(state): State<AppState>,
    json: Json<crate::crawl::models::CrawlRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    post_crawl_handler(State(state.crawler), json).await
}

async fn get_crawl_wrapper(
    State(state): State<AppState>,
    path: axum::extract::Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    get_crawl_handler(State(state.crawler), path).await
}

async fn delete_crawl_wrapper(
    State(state): State<AppState>,
    path: axum::extract::Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    delete_crawl_handler(State(state.crawler), path).await
}

async fn post_extract_wrapper(
    State(state): State<AppState>,
    json: Json<crate::extraction::models::ExtractionRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    post_extract_handler(State(state.extraction), json).await
}

async fn post_extract_batch_wrapper(
    State(state): State<AppState>,
    json: Json<crate::extraction::models::BatchExtractionRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    post_extract_batch_handler(State(state.extraction), json).await
}

async fn get_extract_job_wrapper(
    State(state): State<AppState>,
    path: axum::extract::Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    get_extract_job_handler(State(state.extraction), path).await
}

async fn delete_extract_job_wrapper(
    State(state): State<AppState>,
    path: axum::extract::Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    delete_extract_job_handler(State(state.extraction), path).await
}
