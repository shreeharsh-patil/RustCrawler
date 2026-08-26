use crate::api::routes::AppState;
use crate::crawl::incremental::IncrementalCrawler;
use crate::discovery::models::MapRequest;
use crate::error::CrawlerError;
use crate::search::models::SearchRequest;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;
use tokio_util::sync::CancellationToken;

pub async fn post_map_handler(
    State(state): State<AppState>,
    Json(request): Json<MapRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    let response = state.map_service.map(request).await?;
    Ok((StatusCode::OK, Json(response)))
}

pub async fn post_search_handler(
    State(state): State<AppState>,
    Json(request): Json<SearchRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    let response = state.search_service.search(request).await?;
    Ok((StatusCode::OK, Json(response)))
}

pub async fn post_incremental_crawl_handler(
    State(state): State<AppState>,
    Json(request): Json<crate::crawl::IncrementalCrawlRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    let incremental = IncrementalCrawler::new(
        state.crawler.clone(),
        state.crawler.snapshot_store().clone(),
    );
    let cancel = CancellationToken::new();
    let response = incremental
        .execute_incremental_crawl(request, cancel)
        .await?;
    Ok((StatusCode::OK, Json(response)))
}

pub async fn get_crawl_changes_handler(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    let snapshot = state
        .crawler
        .snapshot_store()
        .get_snapshot(&job_id)
        .ok_or_else(|| CrawlerError::SnapshotNotFound(job_id.clone()))?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "job_id": job_id,
            "snapshot_id": snapshot.id,
            "seed_url": snapshot.seed_url,
            "created_at": snapshot.created_at,
            "total_pages": snapshot.pages.len(),
            "pages": snapshot.pages
        })),
    ))
}
