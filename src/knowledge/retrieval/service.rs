use crate::config::Config;
use crate::error::CrawlerError;
use crate::knowledge::chunking::tokenizer::{ApproximateTokenizer, TokenCounter};
use crate::knowledge::embeddings::EmbeddingBatcher;
use crate::knowledge::hybrid::fusion::reciprocal_rank_fusion;
use crate::knowledge::lexical::LexicalIndex;
use crate::knowledge::models::{HybridMatch, IndexFilter, RetrievalResult, SearchMode};
use crate::knowledge::vector::{VectorQuery, VectorStore};
use std::sync::Arc;

pub struct RetrievalService {
    config: Arc<Config>,
    vector_store: Arc<dyn VectorStore>,
    lexical_index: Arc<LexicalIndex>,
    embedding_batcher: Option<Arc<EmbeddingBatcher>>,
    tokenizer: Arc<dyn TokenCounter>,
}

impl RetrievalService {
    pub fn new(
        config: Arc<Config>,
        vector_store: Arc<dyn VectorStore>,
        lexical_index: Arc<LexicalIndex>,
        embedding_batcher: Option<Arc<EmbeddingBatcher>>,
    ) -> Self {
        Self {
            config,
            vector_store,
            lexical_index,
            embedding_batcher,
            tokenizer: Arc::new(ApproximateTokenizer),
        }
    }

    /// Performs search using the requested mode (Lexical, Semantic, or Hybrid)
    pub async fn search(
        &self,
        index_id: &str,
        query: &str,
        limit: usize,
        mode: SearchMode,
        filter: Option<IndexFilter>,
        tenant_id: Option<String>,
    ) -> Result<Vec<HybridMatch>, CrawlerError> {
        let effective_limit = limit.clamp(1, 100);

        match mode {
            SearchMode::Lexical => Ok(self.search_lexical(
                index_id,
                query,
                effective_limit,
                filter.as_ref(),
                tenant_id.as_deref(),
            )),
            SearchMode::Semantic => {
                let batcher = self
                    .embedding_batcher
                    .as_ref()
                    .ok_or(CrawlerError::EmbeddingsDisabled)?;

                let query_vec = batcher
                    .embed_query(query)
                    .await
                    .map_err(|e| CrawlerError::EmbeddingProviderError(e.to_string()))?;

                let v_query = VectorQuery {
                    vector: query_vec,
                    limit: effective_limit,
                    filter: filter.clone(),
                    tenant_id,
                    min_score: None,
                };

                let vector_results = self
                    .vector_store
                    .search(index_id, v_query)
                    .await
                    .map_err(|e| CrawlerError::VectorStoreError(e.to_string()))?;

                let matches = vector_results
                    .into_iter()
                    .enumerate()
                    .map(|(idx, v)| HybridMatch {
                        chunk_id: v.chunk_id,
                        document_id: v.document_id,
                        source_url: v.source_url,
                        title: v.title,
                        heading_path: v.heading_path,
                        text: v.text,
                        score: v.score,
                        lexical_rank: None,
                        vector_rank: Some(idx + 1),
                        rrf_score: 1.0 / (60.0 + (idx + 1) as f64),
                        metadata: v.metadata,
                        document_metadata: v.document_metadata,
                    })
                    .collect();

                Ok(matches)
            }
            SearchMode::Hybrid => {
                // If embeddings are configured, do vector + lexical fusion; otherwise fallback to lexical
                if let Some(ref batcher) = self.embedding_batcher {
                    let query_vec_res = batcher.embed_query(query).await;
                    match query_vec_res {
                        Ok(query_vec) => {
                            let v_query = VectorQuery {
                                vector: query_vec,
                                limit: effective_limit * 2,
                                filter: filter.clone(),
                                tenant_id: tenant_id.clone(),
                                min_score: None,
                            };

                            let vector_matches = self
                                .vector_store
                                .search(index_id, v_query)
                                .await
                                .unwrap_or_default();

                            let lexical_matches = self.lexical_index.search(
                                index_id,
                                query,
                                effective_limit * 2,
                                filter.as_ref(),
                                tenant_id.as_deref(),
                            );

                            let fused = reciprocal_rank_fusion(
                                &vector_matches,
                                &lexical_matches,
                                60.0,
                                self.config.max_chunks_per_document,
                                effective_limit,
                            );

                            Ok(fused)
                        }
                        Err(_) => {
                            // Graceful fallback to lexical search if embedding provider fails
                            Ok(self.search_lexical(
                                index_id,
                                query,
                                effective_limit,
                                filter.as_ref(),
                                tenant_id.as_deref(),
                            ))
                        }
                    }
                } else {
                    Ok(self.search_lexical(
                        index_id,
                        query,
                        effective_limit,
                        filter.as_ref(),
                        tenant_id.as_deref(),
                    ))
                }
            }
        }
    }

    fn search_lexical(
        &self,
        index_id: &str,
        query: &str,
        limit: usize,
        filter: Option<&IndexFilter>,
        tenant_id: Option<&str>,
    ) -> Vec<HybridMatch> {
        let lexical_results = self
            .lexical_index
            .search(index_id, query, limit, filter, tenant_id);

        lexical_results
            .into_iter()
            .enumerate()
            .map(|(idx, l)| HybridMatch {
                chunk_id: l.chunk_id,
                document_id: l.document_id,
                source_url: l.source_url,
                title: l.title,
                heading_path: l.heading_path,
                text: l.text,
                score: l.score,
                lexical_rank: Some(idx + 1),
                vector_rank: None,
                rrf_score: 1.0 / (60.0 + (idx + 1) as f64),
                metadata: l.metadata,
                document_metadata: l.document_metadata,
            })
            .collect()
    }

    /// Packs retrieved chunks within token budget for RAG
    #[allow(clippy::too_many_arguments)]
    pub async fn retrieve(
        &self,
        index_id: &str,
        query: &str,
        top_k: usize,
        mode: SearchMode,
        filter: Option<IndexFilter>,
        tenant_id: Option<String>,
        query_expansion: bool,
    ) -> Result<RetrievalResult, CrawlerError> {
        let max_chunks = top_k.clamp(1, self.config.rag_max_chunks);

        let mut queries = vec![query.to_string()];
        if query_expansion {
            let expanded = generate_local_query_variations(query, self.config.max_query_expansions);
            queries.extend(expanded);
        }

        let mut all_matches = Vec::new();
        for q in queries {
            if let Ok(matches) = self
                .search(
                    index_id,
                    &q,
                    max_chunks * 2,
                    mode,
                    filter.clone(),
                    tenant_id.clone(),
                )
                .await
            {
                all_matches.extend(matches);
            }
        }

        // Deduplicate chunks across queries by chunk_id
        let mut unique_chunks: Vec<HybridMatch> = Vec::new();
        let mut seen_chunk_ids = std::collections::HashSet::new();

        for m in all_matches {
            if seen_chunk_ids.insert(m.chunk_id.clone()) {
                if let Some(min_score) = self.config.rag_min_score {
                    if m.score < min_score {
                        continue;
                    }
                }
                unique_chunks.push(m);
            }
        }

        // Sort descending by score
        unique_chunks.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Pack into token budget (RAG_MAX_CONTEXT_TOKENS)
        let mut packed_chunks = Vec::new();
        let mut total_tokens = 0;
        let token_budget = self.config.rag_max_context_tokens;

        for chunk in unique_chunks {
            let chunk_tokens = self.tokenizer.count(&chunk.text);
            if total_tokens + chunk_tokens > token_budget && !packed_chunks.is_empty() {
                break;
            }
            total_tokens += chunk_tokens;
            packed_chunks.push(chunk);
            if packed_chunks.len() >= max_chunks {
                break;
            }
        }

        let count = packed_chunks.len();
        Ok(RetrievalResult {
            query: query.to_string(),
            chunks: packed_chunks,
            total_chunks: count,
            total_tokens,
        })
    }
}

/// Generates lightweight deterministic query variations without extra API calls
fn generate_local_query_variations(query: &str, max_expansions: usize) -> Vec<String> {
    let mut variations = Vec::new();
    let words: Vec<&str> = query.split_whitespace().collect();

    // 1. If query is a question ("How do I ...?"), strip question words
    if words.len() > 3 {
        let stripped: Vec<&str> = words
            .iter()
            .copied()
            .filter(|&w| {
                let lw = w.to_lowercase();
                lw != "how"
                    && lw != "what"
                    && lw != "where"
                    && lw != "when"
                    && lw != "why"
                    && lw != "do"
                    && lw != "does"
                    && lw != "i"
                    && lw != "can"
                    && lw != "the"
                    && lw != "a"
                    && lw != "is"
            })
            .collect();
        if !stripped.is_empty() && stripped.len() != words.len() {
            variations.push(stripped.join(" "));
        }
    }

    if variations.len() > max_expansions {
        variations.truncate(max_expansions);
    }

    variations
}
