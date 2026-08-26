use crate::error::CrawlerError;
use crate::extraction::models::{BatchExtractionRequest, ExtractionRequest, ExtractionResponse};
use crate::extraction::service::ExtractionService;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;

pub async fn post_extract_handler(
    State(extraction): State<Arc<ExtractionService>>,
    Json(request): Json<ExtractionRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    let result = extraction.extract(request, None).await?;
    Ok((StatusCode::OK, Json(ExtractionResponse::from(result))))
}

pub async fn post_extract_batch_handler(
    State(extraction): State<Arc<ExtractionService>>,
    Json(request): Json<BatchExtractionRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    let batch_response = extraction.extract_batch(request, None).await;
    Ok((StatusCode::OK, Json(batch_response)))
}

pub async fn get_extract_job_handler(
    State(extraction): State<Arc<ExtractionService>>,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    let job = extraction
        .get_job(&job_id)
        .ok_or(CrawlerError::JobNotFound(job_id))?;
    Ok((StatusCode::OK, Json(job)))
}

pub async fn delete_extract_job_handler(
    State(extraction): State<Arc<ExtractionService>>,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    let deleted = extraction.delete_job(&job_id);
    if deleted {
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "cancelled": true})),
        ))
    } else {
        Err(CrawlerError::JobNotFound(job_id))
    }
}
