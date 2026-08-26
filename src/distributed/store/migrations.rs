pub struct Migration {
    pub version: usize,
    pub name: &'static str,
    pub sql_postgres: &'static str,
    pub sql_sqlite: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "001_initial_schema",
    sql_postgres: r#"
            CREATE TABLE IF NOT EXISTS jobs (
                id VARCHAR(64) PRIMARY KEY,
                job_type VARCHAR(32) NOT NULL,
                status VARCHAR(32) NOT NULL,
                configuration JSONB NOT NULL,
                tenant_id VARCHAR(64),
                created_at BIGINT NOT NULL,
                started_at BIGINT,
                completed_at BIGINT,
                error_summary TEXT,
                result_location TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
            CREATE INDEX IF NOT EXISTS idx_jobs_tenant ON jobs(tenant_id);
            CREATE INDEX IF NOT EXISTS idx_jobs_created_at ON jobs(created_at);

            CREATE TABLE IF NOT EXISTS job_tasks (
                task_id VARCHAR(64) PRIMARY KEY,
                job_id VARCHAR(64) NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
                task_type VARCHAR(32) NOT NULL,
                payload JSONB NOT NULL,
                priority INT NOT NULL DEFAULT 2,
                attempt INT NOT NULL DEFAULT 0,
                max_attempts INT NOT NULL DEFAULT 5,
                status VARCHAR(32) NOT NULL DEFAULT 'queued',
                lease_owner VARCHAR(64),
                lease_expiry BIGINT,
                created_at BIGINT NOT NULL,
                available_at BIGINT NOT NULL,
                error TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_tasks_job_id ON job_tasks(job_id);
            CREATE INDEX IF NOT EXISTS idx_tasks_status_available ON job_tasks(status, available_at);
            CREATE INDEX IF NOT EXISTS idx_tasks_lease_expiry ON job_tasks(lease_expiry);

            CREATE TABLE IF NOT EXISTS job_progress (
                job_id VARCHAR(64) PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE,
                total_discovered INT NOT NULL DEFAULT 0,
                queued_tasks INT NOT NULL DEFAULT 0,
                active_tasks INT NOT NULL DEFAULT 0,
                completed_pages INT NOT NULL DEFAULT 0,
                failed_pages INT NOT NULL DEFAULT 0,
                unchanged_pages INT NOT NULL DEFAULT 0,
                bytes_downloaded BIGINT NOT NULL DEFAULT 0,
                updated_at BIGINT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS crawl_urls (
                id BIGSERIAL PRIMARY KEY,
                job_id VARCHAR(64) NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
                normalized_url TEXT NOT NULL,
                url_hash VARCHAR(64) NOT NULL,
                original_url TEXT NOT NULL,
                depth INT NOT NULL DEFAULT 0,
                status VARCHAR(32) NOT NULL DEFAULT 'discovered',
                discovered_at BIGINT NOT NULL,
                UNIQUE(job_id, url_hash)
            );

            CREATE INDEX IF NOT EXISTS idx_crawl_urls_job_hash ON crawl_urls(job_id, url_hash);

            CREATE TABLE IF NOT EXISTS crawl_results (
                id BIGSERIAL PRIMARY KEY,
                job_id VARCHAR(64) NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
                url TEXT NOT NULL,
                status_code INT NOT NULL,
                content_hash VARCHAR(64),
                normalized_content_hash VARCHAR(64),
                result_location TEXT,
                created_at BIGINT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_crawl_results_job ON crawl_results(job_id);

            CREATE TABLE IF NOT EXISTS snapshots (
                snapshot_id VARCHAR(64) PRIMARY KEY,
                seed_url TEXT NOT NULL,
                created_at BIGINT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS snapshot_pages (
                snapshot_id VARCHAR(64) NOT NULL REFERENCES snapshots(snapshot_id) ON DELETE CASCADE,
                url TEXT NOT NULL,
                final_url TEXT NOT NULL,
                content_hash VARCHAR(64),
                normalized_content_hash VARCHAR(64),
                etag VARCHAR(128),
                last_modified VARCHAR(128),
                status_code INT NOT NULL,
                title TEXT,
                PRIMARY KEY (snapshot_id, url)
            );

            CREATE TABLE IF NOT EXISTS webhook_endpoints (
                id VARCHAR(64) PRIMARY KEY,
                url TEXT NOT NULL,
                secret VARCHAR(128),
                events TEXT[] NOT NULL,
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                created_at BIGINT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS webhook_deliveries (
                delivery_id VARCHAR(64) PRIMARY KEY,
                event_id VARCHAR(64) NOT NULL,
                job_id VARCHAR(64) NOT NULL,
                endpoint_url TEXT NOT NULL,
                event_type VARCHAR(64) NOT NULL,
                status VARCHAR(32) NOT NULL,
                attempt INT NOT NULL DEFAULT 1,
                response_status INT,
                error TEXT,
                next_retry_at BIGINT,
                created_at BIGINT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_webhook_status_retry ON webhook_deliveries(status, next_retry_at);

            CREATE TABLE IF NOT EXISTS workers (
                worker_id VARCHAR(64) PRIMARY KEY,
                worker_type VARCHAR(32) NOT NULL,
                hostname VARCHAR(128) NOT NULL,
                version VARCHAR(32) NOT NULL,
                max_concurrency INT NOT NULL DEFAULT 10,
                active_tasks INT NOT NULL DEFAULT 0,
                browser_available BOOLEAN NOT NULL DEFAULT FALSE,
                llm_available BOOLEAN NOT NULL DEFAULT FALSE,
                started_at BIGINT NOT NULL,
                last_heartbeat BIGINT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_workers_heartbeat ON workers(last_heartbeat);

            CREATE TABLE IF NOT EXISTS system_events (
                event_id VARCHAR(64) PRIMARY KEY,
                job_id VARCHAR(64) NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
                event_type VARCHAR(64) NOT NULL,
                timestamp BIGINT NOT NULL,
                data JSONB NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_events_job_time ON system_events(job_id, timestamp);
        "#,
    sql_sqlite: r#"
            CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                job_type TEXT NOT NULL,
                status TEXT NOT NULL,
                configuration TEXT NOT NULL,
                tenant_id TEXT,
                created_at INTEGER NOT NULL,
                started_at INTEGER,
                completed_at INTEGER,
                error_summary TEXT,
                result_location TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
            CREATE INDEX IF NOT EXISTS idx_jobs_tenant ON jobs(tenant_id);

            CREATE TABLE IF NOT EXISTS job_tasks (
                task_id TEXT PRIMARY KEY,
                job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
                task_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 2,
                attempt INTEGER NOT NULL DEFAULT 0,
                max_attempts INTEGER NOT NULL DEFAULT 5,
                status TEXT NOT NULL DEFAULT 'queued',
                lease_owner TEXT,
                lease_expiry INTEGER,
                created_at INTEGER NOT NULL,
                available_at INTEGER NOT NULL,
                error TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_tasks_job_id ON job_tasks(job_id);
            CREATE INDEX IF NOT EXISTS idx_tasks_status_available ON job_tasks(status, available_at);

            CREATE TABLE IF NOT EXISTS workers (
                worker_id TEXT PRIMARY KEY,
                worker_type TEXT NOT NULL,
                hostname TEXT NOT NULL,
                version TEXT NOT NULL,
                max_concurrency INTEGER NOT NULL DEFAULT 10,
                active_tasks INTEGER NOT NULL DEFAULT 0,
                browser_available INTEGER NOT NULL DEFAULT 0,
                llm_available INTEGER NOT NULL DEFAULT 0,
                started_at INTEGER NOT NULL,
                last_heartbeat INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS system_events (
                event_id TEXT PRIMARY KEY,
                job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
                event_type TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                data TEXT NOT NULL
            );
        "#,
}];

pub fn get_migration_statements(is_postgres: bool) -> Vec<&'static str> {
    MIGRATIONS
        .iter()
        .map(|m| {
            if is_postgres {
                m.sql_postgres
            } else {
                m.sql_sqlite
            }
        })
        .collect()
}
