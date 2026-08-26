use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::Json;
use futures::stream::Stream;
use serde_json::json;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::agent::models::AgentRequest;
use crate::api::routes::AppState;
use crate::error::CrawlerError;

/// Handler for `POST /v1/agent`
pub async fn submit_agent_job(
    State(state): State<AppState>,
    Json(payload): Json<AgentRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    let agent_svc = state
        .agent_service
        .as_ref()
        .ok_or(CrawlerError::AgentDisabled)?;

    let job_id = agent_svc.submit_job(payload).await?;

    Ok(Json(json!({
        "success": true,
        "job_id": job_id,
        "status": "queued",
        "message": "Research job submitted successfully"
    })))
}

/// Handler for `GET /v1/agent/{job_id}`
pub async fn get_agent_job_status(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    let agent_svc = state
        .agent_service
        .as_ref()
        .ok_or(CrawlerError::AgentDisabled)?;

    let summary = agent_svc
        .get_job_summary(&job_id)
        .ok_or(CrawlerError::JobNotFound(job_id))?;

    Ok(Json(json!({
        "success": true,
        "job_id": summary.job_id,
        "task": summary.task,
        "status": summary.status,
        "created_at": summary.created_at,
        "updated_at": summary.updated_at,
        "progress": summary.progress,
        "result": summary.result,
        "error": summary.error
    })))
}

/// Handler for `DELETE /v1/agent/{job_id}`
pub async fn cancel_agent_job(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    let agent_svc = state
        .agent_service
        .as_ref()
        .ok_or(CrawlerError::AgentDisabled)?;

    agent_svc.cancel_job(&job_id)?;

    Ok(Json(json!({
        "success": true,
        "job_id": job_id,
        "status": "cancelled",
        "message": "Research job was cancelled"
    })))
}

/// Handler for `POST /v1/agent/{job_id}/pause`
pub async fn pause_agent_job(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    let agent_svc = state
        .agent_service
        .as_ref()
        .ok_or(CrawlerError::AgentDisabled)?;

    agent_svc.pause_job(&job_id)?;

    Ok(Json(json!({
        "success": true,
        "job_id": job_id,
        "status": "paused",
        "message": "Research job paused"
    })))
}

/// Handler for `POST /v1/agent/{job_id}/resume`
pub async fn resume_agent_job(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    let agent_svc = state
        .agent_service
        .as_ref()
        .ok_or(CrawlerError::AgentDisabled)?;

    agent_svc.resume_job(&job_id)?;

    Ok(Json(json!({
        "success": true,
        "job_id": job_id,
        "status": "resumed",
        "message": "Research job resumed"
    })))
}

/// Handler for `GET /v1/agent/{job_id}/events` (SSE)
pub async fn stream_agent_events(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, CrawlerError> {
    let rx = state.scheduler.subscribe_events();
    let stream = BroadcastStream::new(rx).filter_map(move |item| {
        let jid = job_id.clone();
        match item {
            Ok(event) => {
                if event.job_id == jid {
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

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}
