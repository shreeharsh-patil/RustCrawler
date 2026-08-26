use crate::crawl::models::{
    CrawlJobInitResponse, CrawlJobStatusResponse, CrawlRequest, CrawlStatus,
};
use crate::crawl::CrawlerService;
use crate::error::CrawlerError;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;

pub async fn post_crawl_handler(
    State(crawler): State<Arc<CrawlerService>>,
    Json(request): Json<CrawlRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    let job = crawler.start_job(request.url, request.options)?;

    Ok((
        StatusCode::ACCEPTED,
        Json(CrawlJobInitResponse {
            success: true,
            job_id: job.job_id.clone(),
            status: CrawlStatus::Queued,
        }),
    ))
}

pub async fn get_crawl_handler(
    State(crawler): State<Arc<CrawlerService>>,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    let job = crawler.get_job(&job_id)?;
    let snapshot = job.to_result_snapshot().await;

    Ok((
        StatusCode::OK,
        Json(CrawlJobStatusResponse {
            success: true,
            data: snapshot,
        }),
    ))
}

pub async fn delete_crawl_handler(
    State(crawler): State<Arc<CrawlerService>>,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    let _job = crawler.cancel_job(&job_id)?;

    Ok((
        StatusCode::OK,
        Json(CrawlJobInitResponse {
            success: true,
            job_id,
            status: CrawlStatus::Cancelled,
        }),
    ))
}
