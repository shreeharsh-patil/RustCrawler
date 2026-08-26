/// SQL DDL migrations and query builders for PostgreSQL with pgvector extension
pub struct PgVectorMigrations;

impl PgVectorMigrations {
    /// Generates PostgreSQL schema DDL for pgvector tables and HNSW vector index
    pub fn ddl_schema(dimensions: usize) -> String {
        format!(
            r#"
-- 1. Ensure pgvector extension exists
CREATE EXTENSION IF NOT EXISTS vector;

-- 2. Table storing vector records for indexed chunks
CREATE TABLE IF NOT EXISTS search_indexes (
    id VARCHAR(64) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    tenant_id VARCHAR(64),
    embedding_model VARCHAR(128),
    embedding_dimensions INTEGER,
    document_count BIGINT NOT NULL DEFAULT 0,
    chunk_count BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_search_indexes_name ON search_indexes(name);
CREATE INDEX IF NOT EXISTS idx_search_indexes_tenant ON search_indexes(tenant_id);

CREATE TABLE IF NOT EXISTS chunk_vectors (
    chunk_id VARCHAR(128) PRIMARY KEY,
    document_id VARCHAR(128) NOT NULL,
    index_id VARCHAR(64) NOT NULL REFERENCES search_indexes(id) ON DELETE CASCADE,
    tenant_id VARCHAR(64),
    source_url TEXT,
    title TEXT,
    heading_path JSONB NOT NULL DEFAULT '[]'::jsonb,
    text TEXT NOT NULL,
    embedding vector({dimensions}) NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{{}}'::jsonb,
    document_metadata JSONB NOT NULL DEFAULT '{{}}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_chunk_vectors_doc ON chunk_vectors(document_id);
CREATE INDEX IF NOT EXISTS idx_chunk_vectors_index ON chunk_vectors(index_id);
CREATE INDEX IF NOT EXISTS idx_chunk_vectors_tenant ON chunk_vectors(tenant_id);

-- 3. HNSW vector index for high-speed cosine similarity search
CREATE INDEX IF NOT EXISTS idx_chunk_vectors_embedding_hnsw 
ON chunk_vectors 
USING hnsw (embedding vector_cosine_ops)
WITH (m = 16, ef_construction = 64);
"#
        )
    }

    /// Generates SQL query string for parameterized cosine similarity search
    pub fn build_search_query(has_filter: bool, has_tenant: bool) -> String {
        let mut where_clauses = vec!["index_id = $1".to_string()];
        let mut param_idx = 3; // $1 is index_id, $2 is query_vector

        if has_tenant {
            where_clauses.push(format!("tenant_id = ${param_idx}"));
            param_idx += 1;
        }

        if has_filter {
            where_clauses.push(format!(
                "(metadata @> ${param_idx} OR document_metadata @> ${param_idx})"
            ));
            // param_idx += 1;
        }

        let where_str = where_clauses.join(" AND ");

        format!(
            r#"
SELECT 
    chunk_id,
    document_id,
    source_url,
    title,
    heading_path,
    text,
    1 - (embedding <=> $2::vector) AS score,
    metadata,
    document_metadata
FROM chunk_vectors
WHERE {where_str}
ORDER BY embedding <=> $2::vector ASC
LIMIT $limit;
"#
        )
    }
}
