use crate::api::routes::AppState;
use crate::error::CrawlerError;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::Json;
use futures::stream::Stream;
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

#[derive(Debug, Deserialize)]
pub struct ResultsPaginationQuery {
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ListJobsQuery {
    pub status: Option<String>,
    pub limit: Option<usize>,
}

pub async fn get_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    if let Some(job) = state
        .metadata_store
        .get_job(&id)
        .await
        .map_err(|e| CrawlerError::Internal(e.to_string()))?
    {
        Ok((StatusCode::OK, Json(json!({ "success": true, "job": job }))))
    } else {
        Err(CrawlerError::JobNotFound(id))
    }
}

pub async fn pause_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    state
        .scheduler
        .pause_job(&id)
        .await
        .map_err(CrawlerError::Internal)?;

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "job_id": id, "status": "paused" })),
    ))
}

pub async fn resume_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    state
        .scheduler
        .resume_job(&id)
        .await
        .map_err(CrawlerError::Internal)?;

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "job_id": id, "status": "running" })),
    ))
}

pub async fn cancel_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    state
        .scheduler
        .cancel_job(&id)
        .await
        .map_err(CrawlerError::Internal)?;

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "job_id": id, "status": "cancelled" })),
    ))
}

pub async fn get_job_results_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<ResultsPaginationQuery>,
) -> Result<impl IntoResponse, CrawlerError> {
    let limit = params.limit.unwrap_or(100).clamp(1, 1000);
    let locations = state
        .result_store
        .list_chunk_locations(&id)
        .await
        .map_err(|e| CrawlerError::Internal(e.to_string()))?;

    let start_idx = if let Some(ref cur) = params.cursor {
        locations
            .iter()
            .position(|l| l == cur)
            .map(|i| i + 1)
            .unwrap_or(0)
    } else {
        0
    };

    let end_idx = (start_idx + limit).min(locations.len());
    let page_locations = if start_idx < locations.len() {
        &locations[start_idx..end_idx]
    } else {
        &[]
    };

    let next_cursor = if end_idx < locations.len() {
        locations.get(end_idx - 1).cloned()
    } else {
        None
    };

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "job_id": id,
            "total_items": locations.len(),
            "returned_items": page_locations.len(),
            "next_cursor": next_cursor,
            "items": page_locations
        })),
    ))
}

pub async fn get_job_events_sse_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.scheduler.subscribe_events();
    let stream = BroadcastStream::new(rx).filter_map(move |item| {
        let job_id = id.clone();
        match item {
            Ok(event) => {
                if event.job_id == job_id {
                    let data_str = serde_json::to_string(&event).unwrap_or_default();
                    Some(Ok(Event::default()
                        .event(format!("{:?}", event.event_type))
                        .data(data_str)))
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

pub async fn readiness_handler(State(state): State<AppState>) -> impl IntoResponse {
    let mode = state.config.execution_mode.as_str();
    let db_ok = if state.config.database_url.is_some() {
        "ok"
    } else {
        "local_ok"
    };
    let redis_ok = if state.config.redis_url.is_some() {
        "ok"
    } else {
        "local_ok"
    };

    (
        StatusCode::OK,
        Json(json!({
            "status": "ready",
            "execution_mode": mode,
            "database": db_ok,
            "redis": redis_ok,
            "object_storage": "ok"
        })),
    )
}
