use crate::api::routes::AppState;
use crate::error::CrawlerError;
use crate::knowledge::models::{IndexFilter, SearchMode};
use crate::models::ScrapeRequest;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreateOrIndexRequest {
    pub name: String,
    #[serde(default)]
    pub embedding_model: Option<String>,
    #[serde(default)]
    pub embedding_dimensions: Option<usize>,
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub markdown: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IndexCrawlRequest {
    pub crawl_job_id: String,
    pub index: String,
    #[serde(default = "default_true")]
    pub incremental: bool,
    #[serde(default)]
    pub tenant_id: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct SemanticSearchRequest {
    pub index: String,
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
    #[serde(default)]
    pub filter: Option<IndexFilter>,
    #[serde(default)]
    pub tenant_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HybridSearchRequest {
    pub index: String,
    pub query: String,
    #[serde(default = "default_hybrid_limit")]
    pub limit: usize,
    #[serde(default)]
    pub filter: Option<IndexFilter>,
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub explain: bool,
}

#[derive(Debug, Deserialize)]
pub struct RetrieveApiRequest {
    pub index: String,
    pub query: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub filter: Option<IndexFilter>,
    #[serde(default)]
    pub query_expansion: bool,
    #[serde(default)]
    pub tenant_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AnswerApiRequest {
    pub index: String,
    pub question: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(default)]
    pub filter: Option<IndexFilter>,
    #[serde(default)]
    pub tenant_id: Option<String>,
}

fn default_search_limit() -> usize {
    10
}
fn default_hybrid_limit() -> usize {
    20
}
fn default_top_k() -> usize {
    8
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: T,
}

/// Handler for POST /v1/index (Create index or index content immediately)
pub async fn create_or_index_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateOrIndexRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    let index = match state.knowledge_engine.get_index(&payload.name) {
        Ok(idx) => idx,
        Err(_) => state.knowledge_engine.create_index(
            &payload.name,
            payload.embedding_model,
            payload.embedding_dimensions,
            payload.tenant_id.clone(),
        )?,
    };

    if let Some(ref url_str) = payload.url {
        let scrape_req = ScrapeRequest::new(url_str);
        let scrape_res = state.scraper.scrape(scrape_req).await?;
        if let Some(md) = scrape_res.markdown {
            let title = scrape_res.metadata.as_ref().and_then(|m| m.title.clone());
            state
                .knowledge_engine
                .index_markdown(&index.id, &md, title.as_deref(), Some(url_str), None)
                .await?;
        }
    } else if let Some(ref md) = payload.markdown {
        state
            .knowledge_engine
            .index_markdown(&index.id, md, payload.title.as_deref(), None, None)
            .await?;
    }

    let stats = state.knowledge_engine.index_stats(&index.id)?;
    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            data: stats,
        }),
    ))
}

/// Handler for POST /v1/index/crawl (Indexes results from a completed crawl job)
pub async fn index_crawl_handler(
    State(state): State<AppState>,
    Json(payload): Json<IndexCrawlRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    let index = match state.knowledge_engine.get_index(&payload.index) {
        Ok(idx) => idx,
        Err(_) => state.knowledge_engine.create_index(
            &payload.index,
            None,
            None,
            payload.tenant_id.clone(),
        )?,
    };

    let mut indexed_count = 0;

    // Retrieve crawl results from ResultStore
    if let Ok(locations) = state
        .result_store
        .list_chunk_locations(&payload.crawl_job_id)
        .await
    {
        for loc in locations {
            if let Ok(bytes) = state.result_store.get(&loc).await {
                if let Ok(scrape_res) =
                    serde_json::from_slice::<crate::models::ScrapeResult>(&bytes)
                {
                    if let Some(ref md) = scrape_res.markdown {
                        let url_str = scrape_res.url.clone();
                        let title = scrape_res.metadata.as_ref().and_then(|m| m.title.clone());
                        let _ = state
                            .knowledge_engine
                            .index_markdown(&index.id, md, title.as_deref(), Some(&url_str), None)
                            .await;
                        indexed_count += 1;
                    }
                }
            }
        }
    }

    let stats = state.knowledge_engine.index_stats(&index.id)?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "crawl_job_id": payload.crawl_job_id,
            "index": index.name,
            "indexed_pages": indexed_count,
            "stats": stats
        })),
    ))
}

/// Handler for POST /v1/search/semantic
pub async fn search_semantic_handler(
    State(state): State<AppState>,
    Json(payload): Json<SemanticSearchRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    let results = state
        .knowledge_engine
        .search(
            &payload.index,
            &payload.query,
            payload.limit,
            SearchMode::Semantic,
            payload.filter,
            payload.tenant_id,
        )
        .await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            data: results,
        }),
    ))
}

/// Handler for POST /v1/search/hybrid
pub async fn search_hybrid_handler(
    State(state): State<AppState>,
    Json(payload): Json<HybridSearchRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    let results = state
        .knowledge_engine
        .search(
            &payload.index,
            &payload.query,
            payload.limit,
            SearchMode::Hybrid,
            payload.filter,
            payload.tenant_id,
        )
        .await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            data: results,
        }),
    ))
}

/// Handler for POST /v1/retrieve
pub async fn retrieve_handler(
    State(state): State<AppState>,
    Json(payload): Json<RetrieveApiRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    let search_mode = match payload.mode.as_deref() {
        Some("semantic") => SearchMode::Semantic,
        Some("lexical") => SearchMode::Lexical,
        _ => SearchMode::Hybrid,
    };

    let result = state
        .knowledge_engine
        .retrieve(
            &payload.index,
            &payload.query,
            payload.top_k,
            search_mode,
            payload.filter,
            payload.tenant_id,
            payload.query_expansion,
        )
        .await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            data: result,
        }),
    ))
}

/// Handler for POST /v1/answer (Grounded RAG)
pub async fn answer_handler(
    State(state): State<AppState>,
    Json(payload): Json<AnswerApiRequest>,
) -> Result<impl IntoResponse, CrawlerError> {
    let result = state
        .knowledge_engine
        .answer(
            &payload.index,
            &payload.question,
            payload.top_k,
            payload.filter,
            payload.tenant_id,
        )
        .await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            data: result,
        }),
    ))
}

/// Handler for GET /v1/index/{index_id}
pub async fn get_index_stats_handler(
    State(state): State<AppState>,
    Path(index_id): Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    let stats = state.knowledge_engine.index_stats(&index_id)?;
    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            data: stats,
        }),
    ))
}

/// Handler for DELETE /v1/index/{index_id}
pub async fn delete_index_handler(
    State(state): State<AppState>,
    Path(index_id): Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    state.knowledge_engine.delete_index(&index_id).await?;
    Ok((
        StatusCode::OK,
        Json(
            serde_json::json!({ "success": true, "message": format!("Index '{index_id}' deleted") }),
        ),
    ))
}

/// Handler for POST /v1/index/{index_id}/rebuild
pub async fn rebuild_index_handler(
    State(state): State<AppState>,
    Path(index_id): Path<String>,
) -> Result<impl IntoResponse, CrawlerError> {
    state.knowledge_engine.rebuild_index(&index_id).await?;
    let stats = state.knowledge_engine.index_stats(&index_id)?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "success": true, "message": "Index rebuilt", "stats": stats })),
    ))
}
