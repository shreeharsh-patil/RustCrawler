use crate::config::Config;
use crate::error::CrawlerError;
use crate::knowledge::chunking::dedupe::compute_chunk_hash;
use crate::knowledge::chunking::structural::{ChunkingConfig, StructuralChunker};
use crate::knowledge::chunking::tokenizer::{ApproximateTokenizer, TokenCounter};
use crate::knowledge::embeddings::{
    BatcherConfig, EmbeddingBatcher, EmbeddingCache, EmbeddingProvider, MockEmbeddingProvider,
    OpenAiCompatibleEmbeddingProvider,
};
use crate::knowledge::lexical::LexicalIndex;
use crate::knowledge::models::{
    HybridMatch, IndexFilter, IndexMetadata, IndexStats, IndexedDocument, RagAnswer,
    RetrievalResult, SearchIndex, SearchMode,
};
use crate::knowledge::rag::RagAnswerService;
use crate::knowledge::retrieval::RetrievalService;
use crate::knowledge::vector::{MemoryVectorStore, VectorRecord, VectorStore};
use chrono::Utc;
use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Metrics tracked by the KnowledgeEngine
#[derive(Debug, Default)]
pub struct KnowledgeMetrics {
    pub documents_indexed: AtomicU64,
    pub chunks_created: AtomicU64,
    pub chunks_embedded: AtomicU64,
    pub incremental_chunks_reused: AtomicU64,
    pub total_searches: AtomicU64,
    pub total_answers: AtomicU64,
}

/// Central knowledge engine orchestrating semantic indexing, hybrid retrieval, and grounded RAG
#[allow(dead_code)]
pub struct KnowledgeEngine {
    config: Arc<Config>,
    indexes: Arc<DashMap<String, SearchIndex>>, // id -> SearchIndex
    name_to_id: Arc<DashMap<String, String>>,   // name -> id
    documents: Arc<DashMap<String, IndexedDocument>>, // document_id -> IndexedDocument
    vector_store: Arc<dyn VectorStore>,
    lexical_index: Arc<LexicalIndex>,
    embedding_batcher: Option<Arc<EmbeddingBatcher>>,
    embedding_cache: Arc<EmbeddingCache>,
    chunker: StructuralChunker,
    tokenizer: Arc<dyn TokenCounter>,
    retrieval_service: Arc<RetrievalService>,
    rag_service: Arc<RagAnswerService>,
    pub metrics: Arc<KnowledgeMetrics>,
}

impl KnowledgeEngine {
    pub fn new(
        config: Arc<Config>,
        llm_provider: Option<Arc<dyn crate::extraction::llm::LlmProvider>>,
    ) -> Self {
        let vector_store: Arc<dyn VectorStore> = Arc::new(MemoryVectorStore::new());
        let lexical_index = Arc::new(LexicalIndex::new());
        let embedding_cache = Arc::new(EmbeddingCache::default());

        // Initialize embedding provider if enabled
        let embedding_batcher = if config.embeddings_enabled || config.semantic_indexing_enabled {
            let provider: Arc<dyn EmbeddingProvider> = match config.embedding_provider.as_str() {
                "openai" | "ollama" => {
                    let base_url = config
                        .embedding_base_url
                        .clone()
                        .unwrap_or_else(|| "https://api.openai.com".to_string());
                    let model = config
                        .embedding_model
                        .clone()
                        .unwrap_or_else(|| "text-embedding-3-small".to_string());
                    let dim = config.embedding_dimensions.unwrap_or(1536);
                    Arc::new(OpenAiCompatibleEmbeddingProvider::new(
                        base_url,
                        config.embedding_api_key.clone(),
                        model,
                        dim,
                        Duration::from_secs(config.embedding_timeout_seconds),
                    ))
                }
                _ => {
                    let dim = config.embedding_dimensions.unwrap_or(1536);
                    Arc::new(MockEmbeddingProvider::new(dim))
                }
            };

            let batcher_cfg = BatcherConfig {
                max_batch_size: config.embedding_batch_size,
                max_batch_tokens: config.max_embedding_batch_tokens,
                max_concurrent_requests: config.max_concurrent_embedding_requests,
            };

            Some(Arc::new(EmbeddingBatcher::new(
                provider,
                embedding_cache.clone(),
                batcher_cfg,
            )))
        } else {
            None
        };

        let chunking_cfg = ChunkingConfig {
            target_tokens: config.target_chunk_tokens,
            max_tokens: config.max_chunk_tokens,
            min_tokens: config.min_chunk_tokens,
            overlap_tokens: config.chunk_overlap_tokens,
        };

        let tokenizer: Arc<dyn TokenCounter> = Arc::new(ApproximateTokenizer);
        let chunker = StructuralChunker::new(chunking_cfg, tokenizer.clone());

        let retrieval_service = Arc::new(RetrievalService::new(
            config.clone(),
            vector_store.clone(),
            lexical_index.clone(),
            embedding_batcher.clone(),
        ));

        let rag_service = Arc::new(RagAnswerService::new(
            config.clone(),
            retrieval_service.clone(),
            llm_provider,
        ));

        Self {
            config,
            indexes: Arc::new(DashMap::new()),
            name_to_id: Arc::new(DashMap::new()),
            documents: Arc::new(DashMap::new()),
            vector_store,
            lexical_index,
            embedding_batcher,
            embedding_cache,
            chunker,
            tokenizer,
            retrieval_service,
            rag_service,
            metrics: Arc::new(KnowledgeMetrics::default()),
        }
    }

    /// Creates a new search index
    pub fn create_index(
        &self,
        name: &str,
        embedding_model: Option<String>,
        embedding_dimensions: Option<usize>,
        tenant_id: Option<String>,
    ) -> Result<SearchIndex, CrawlerError> {
        if self.name_to_id.contains_key(name) {
            return Err(CrawlerError::IndexAlreadyExists(name.to_string()));
        }

        let index = SearchIndex::new(name, embedding_model, embedding_dimensions, tenant_id);
        self.name_to_id.insert(name.to_string(), index.id.clone());
        self.indexes.insert(index.id.clone(), index.clone());

        Ok(index)
    }

    /// Resolves an index by name or ID
    pub fn get_index(&self, name_or_id: &str) -> Result<SearchIndex, CrawlerError> {
        if let Some(idx) = self.indexes.get(name_or_id) {
            return Ok(idx.clone());
        }
        if let Some(id) = self.name_to_id.get(name_or_id) {
            if let Some(idx) = self.indexes.get(id.value()) {
                return Ok(idx.clone());
            }
        }
        Err(CrawlerError::IndexNotFound(name_or_id.to_string()))
    }

    /// Lists all indexes for a tenant
    pub fn list_indexes(&self, tenant_id: Option<&str>) -> Vec<SearchIndex> {
        self.indexes
            .iter()
            .filter(|entry| {
                if let Some(t) = tenant_id {
                    entry.value().tenant_id.as_deref() == Some(t)
                } else {
                    true
                }
            })
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Deletes an index and all its indexed chunks and vectors
    pub async fn delete_index(&self, name_or_id: &str) -> Result<(), CrawlerError> {
        let index = self.get_index(name_or_id)?;
        self.indexes.remove(&index.id);
        self.name_to_id.remove(&index.name);
        self.vector_store
            .delete_index(&index.id)
            .await
            .map_err(|e| CrawlerError::VectorStoreError(e.to_string()))?;
        self.lexical_index.delete_index(&index.id);
        Ok(())
    }

    /// Ingests and indexes an IndexedDocument into the target index using incremental chunking
    pub async fn index_document(
        &self,
        index_id_or_name: &str,
        doc: IndexedDocument,
    ) -> Result<IndexedDocument, CrawlerError> {
        let index = self.get_index(index_id_or_name)?;
        let chunks = doc.chunks.clone();

        if chunks.is_empty() {
            return Ok(doc);
        }

        // 1. Incremental change detection against previously indexed version of document
        let old_doc_opt = self.documents.get(&doc.document_id).map(|d| d.clone());
        let mut old_chunk_hashes: HashSet<String> = HashSet::new();
        if let Some(ref old_doc) = old_doc_opt {
            for c in &old_doc.chunks {
                old_chunk_hashes.insert(c.content_hash.clone());
            }
        }

        let mut new_or_modified_chunks = Vec::new();
        let mut reused_chunk_count = 0;

        for chunk in &chunks {
            if old_chunk_hashes.contains(&chunk.content_hash) {
                reused_chunk_count += 1;
            } else {
                new_or_modified_chunks.push(chunk.clone());
            }
        }

        self.metrics
            .incremental_chunks_reused
            .fetch_add(reused_chunk_count as u64, Ordering::Relaxed);

        // 2. Generate embeddings for new/modified chunks if batcher is active
        let mut vector_records = Vec::with_capacity(chunks.len());
        if let Some(ref batcher) = self.embedding_batcher {
            let embeddings = batcher
                .embed_chunks(&chunks)
                .await
                .map_err(|e| CrawlerError::EmbeddingProviderError(e.to_string()))?;

            self.metrics
                .chunks_embedded
                .fetch_add(new_or_modified_chunks.len() as u64, Ordering::Relaxed);

            for (chunk, vec) in chunks.iter().zip(embeddings) {
                vector_records.push(VectorRecord {
                    chunk_id: chunk.chunk_id.clone(),
                    document_id: doc.document_id.clone(),
                    index_id: index.id.clone(),
                    tenant_id: index.tenant_id.clone(),
                    source_url: doc.source_url.clone(),
                    title: doc.title.clone(),
                    heading_path: chunk.heading_path.clone(),
                    text: chunk.text.clone(),
                    vector: vec,
                    metadata: chunk.metadata.clone(),
                    document_metadata: Some(doc.metadata.clone()),
                });
            }

            // Upsert vectors
            self.vector_store
                .upsert(&index.id, vector_records)
                .await
                .map_err(|e| CrawlerError::VectorStoreError(e.to_string()))?;
        }

        // 3. Upsert lexical inverted index entries
        for chunk in &chunks {
            self.lexical_index.insert_chunk(
                &index.id,
                chunk,
                doc.title.as_deref(),
                index.tenant_id.as_deref(),
                Some(&doc.metadata),
            );
        }

        // 4. Update document registry and index stats
        self.documents.insert(doc.document_id.clone(), doc.clone());
        self.metrics
            .documents_indexed
            .fetch_add(1, Ordering::Relaxed);
        self.metrics
            .chunks_created
            .fetch_add(chunks.len() as u64, Ordering::Relaxed);

        if let Some(mut idx_entry) = self.indexes.get_mut(&index.id) {
            idx_entry.document_count += 1;
            idx_entry.chunk_count += chunks.len() as u64;
            idx_entry.updated_at = Utc::now();
        }

        Ok(doc)
    }

    /// Indexes raw markdown text into the specified index
    pub async fn index_markdown(
        &self,
        index_id_or_name: &str,
        markdown: &str,
        title: Option<&str>,
        source_url: Option<&str>,
        metadata: Option<IndexMetadata>,
    ) -> Result<IndexedDocument, CrawlerError> {
        let doc_id = format!("doc_{}", uuid::Uuid::new_v4().simple());
        let content_hash = compute_chunk_hash(markdown);
        let chunks = self.chunker.chunk_markdown(&doc_id, source_url, markdown);

        let mut meta = metadata.unwrap_or_default();
        if meta.title.is_none() {
            meta.title = title.map(ToString::to_string);
        }

        let doc = IndexedDocument {
            document_id: doc_id,
            source_url: source_url.map(ToString::to_string),
            content_hash,
            title: title.map(ToString::to_string),
            content_type: "markdown".to_string(),
            metadata: meta,
            chunks,
        };

        self.index_document(index_id_or_name, doc).await
    }

    /// Indexes structured JSON into the specified index
    pub async fn index_json_value(
        &self,
        index_id_or_name: &str,
        json_val: &serde_json::Value,
        title: Option<&str>,
        source_url: Option<&str>,
        metadata: Option<IndexMetadata>,
    ) -> Result<IndexedDocument, CrawlerError> {
        let doc_id = format!("doc_{}", uuid::Uuid::new_v4().simple());
        let json_str = serde_json::to_string(json_val).unwrap_or_default();
        let content_hash = compute_chunk_hash(&json_str);
        let chunks = self.chunker.chunk_json(&doc_id, source_url, json_val);

        let mut meta = metadata.unwrap_or_default();
        if meta.title.is_none() {
            meta.title = title.map(ToString::to_string);
        }

        let doc = IndexedDocument {
            document_id: doc_id,
            source_url: source_url.map(ToString::to_string),
            content_hash,
            title: title.map(ToString::to_string),
            content_type: "json".to_string(),
            metadata: meta,
            chunks,
        };

        self.index_document(index_id_or_name, doc).await
    }

    /// Indexes PDF extracted page texts into the specified index
    pub async fn index_pdf_pages(
        &self,
        index_id_or_name: &str,
        pages: &[(usize, String)],
        title: Option<&str>,
        source_url: Option<&str>,
        metadata: Option<IndexMetadata>,
    ) -> Result<IndexedDocument, CrawlerError> {
        let doc_id = format!("doc_{}", uuid::Uuid::new_v4().simple());
        let combined_text: String = pages
            .iter()
            .map(|(_, t)| t.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let content_hash = compute_chunk_hash(&combined_text);
        let chunks = self.chunker.chunk_pdf_pages(&doc_id, source_url, pages);

        let mut meta = metadata.unwrap_or_default();
        if meta.title.is_none() {
            meta.title = title.map(ToString::to_string);
        }

        let doc = IndexedDocument {
            document_id: doc_id,
            source_url: source_url.map(ToString::to_string),
            content_hash,
            title: title.map(ToString::to_string),
            content_type: "pdf".to_string(),
            metadata: meta,
            chunks,
        };

        self.index_document(index_id_or_name, doc).await
    }

    /// Searches the specified index
    pub async fn search(
        &self,
        index_id_or_name: &str,
        query: &str,
        limit: usize,
        mode: SearchMode,
        filter: Option<IndexFilter>,
        tenant_id: Option<String>,
    ) -> Result<Vec<HybridMatch>, CrawlerError> {
        let index = self.get_index(index_id_or_name)?;
        self.metrics.total_searches.fetch_add(1, Ordering::Relaxed);
        self.retrieval_service
            .search(&index.id, query, limit, mode, filter, tenant_id)
            .await
    }

    /// Retrieves context-packed chunks for RAG
    #[allow(clippy::too_many_arguments)]
    pub async fn retrieve(
        &self,
        index_id_or_name: &str,
        query: &str,
        top_k: usize,
        mode: SearchMode,
        filter: Option<IndexFilter>,
        tenant_id: Option<String>,
        query_expansion: bool,
    ) -> Result<RetrievalResult, CrawlerError> {
        let index = self.get_index(index_id_or_name)?;
        self.retrieval_service
            .retrieve(
                &index.id,
                query,
                top_k,
                mode,
                filter,
                tenant_id,
                query_expansion,
            )
            .await
    }

    /// Answers a question grounded in retrieved indexed chunks
    pub async fn answer(
        &self,
        index_id_or_name: &str,
        question: &str,
        top_k: usize,
        filter: Option<IndexFilter>,
        tenant_id: Option<String>,
    ) -> Result<RagAnswer, CrawlerError> {
        let index = self.get_index(index_id_or_name)?;
        self.metrics.total_answers.fetch_add(1, Ordering::Relaxed);
        self.rag_service
            .answer(&index.id, question, top_k, filter, tenant_id)
            .await
    }

    /// Rebuilds an index from all stored documents
    pub async fn rebuild_index(&self, index_id_or_name: &str) -> Result<(), CrawlerError> {
        let index = self.get_index(index_id_or_name)?;
        self.vector_store
            .delete_index(&index.id)
            .await
            .map_err(|e| CrawlerError::VectorStoreError(e.to_string()))?;
        self.lexical_index.delete_index(&index.id);

        let docs_to_reindex: Vec<IndexedDocument> =
            self.documents.iter().map(|d| d.value().clone()).collect();

        for doc in docs_to_reindex {
            self.index_document(&index.id, doc).await?;
        }

        Ok(())
    }

    /// Returns statistics for the index
    pub fn index_stats(&self, name_or_id: &str) -> Result<IndexStats, CrawlerError> {
        let index = self.get_index(name_or_id)?;
        let total_tokens: u64 = self
            .documents
            .iter()
            .map(|d| d.chunks.iter().map(|c| c.token_count as u64).sum::<u64>())
            .sum();

        let avg_tokens = if index.chunk_count > 0 {
            total_tokens as f64 / index.chunk_count as f64
        } else {
            0.0
        };

        Ok(IndexStats {
            index: index.clone(),
            document_count: index.document_count,
            chunk_count: index.chunk_count,
            total_tokens,
            average_chunk_tokens: avg_tokens,
        })
    }
}
