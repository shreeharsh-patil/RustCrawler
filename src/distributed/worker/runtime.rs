use crate::config::Config;
use crate::distributed::coordinator::frontier::DistributedFrontier;
use crate::distributed::limiter::DistributedHostLimiter;
use crate::distributed::models::{
    QueuedTask, SystemEventRecord, SystemEventType, TaskFailure, TaskLease, TaskPriority,
    TaskStatus, TaskType, WorkerInfo, WorkerType,
};
use crate::distributed::queue::TaskQueue;
use crate::distributed::results::ResultStore;
use crate::distributed::store::MetadataStore;
use crate::models::{OutputFormat, ScrapeRequest};
use crate::service::ScraperService;
use chrono::Utc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use url::Url;
use uuid::Uuid;

pub struct WorkerRuntime {
    worker_id: String,
    worker_type: WorkerType,
    config: Arc<Config>,
    scraper: Arc<ScraperService>,
    store: Arc<dyn MetadataStore>,
    queue: Arc<dyn TaskQueue>,
    results: Arc<dyn ResultStore>,
    frontier: Arc<DistributedFrontier>,
    limiter: Arc<DistributedHostLimiter>,
    concurrency_limit: usize,
    active_tasks: Arc<AtomicUsize>,
}

impl WorkerRuntime {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        worker_id: impl Into<String>,
        worker_type: WorkerType,
        config: Arc<Config>,
        scraper: Arc<ScraperService>,
        store: Arc<dyn MetadataStore>,
        queue: Arc<dyn TaskQueue>,
        results: Arc<dyn ResultStore>,
        frontier: Arc<DistributedFrontier>,
        concurrency_limit: usize,
    ) -> Self {
        Self {
            worker_id: worker_id.into(),
            worker_type,
            config,
            scraper,
            store,
            queue,
            results,
            frontier,
            limiter: Arc::new(DistributedHostLimiter::new()),
            concurrency_limit: concurrency_limit.clamp(1, 500),
            active_tasks: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub async fn run(&self, cancel_token: CancellationToken) {
        let now = Utc::now().timestamp() as u64;
        let worker_info = WorkerInfo {
            worker_id: self.worker_id.clone(),
            worker_type: self.worker_type,
            hostname: "localhost".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            max_concurrency: self.concurrency_limit,
            active_tasks: 0,
            browser_available: self.scraper.browser_manager().is_enabled(),
            llm_available: self.config.llm_enabled,
            started_at: now,
            last_heartbeat: now,
        };

        if let Err(e) = self.store.register_worker(worker_info).await {
            error!("Failed to register worker in store: {e}");
        }

        info!(
            "Worker [{}] (type: {:?}) started with concurrency {}",
            self.worker_id, self.worker_type, self.concurrency_limit
        );

        let semaphore = Arc::new(Semaphore::new(self.concurrency_limit));

        // 1. Spawn Heartbeat Task
        let heartbeat_store = self.store.clone();
        let heartbeat_wid = self.worker_id.clone();
        let heartbeat_active = self.active_tasks.clone();
        let heartbeat_cancel = cancel_token.clone();
        let heartbeat_secs = self.config.worker_heartbeat_seconds.max(5);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(heartbeat_secs));
            while !heartbeat_cancel.is_cancelled() {
                interval.tick().await;
                let active = heartbeat_active.load(Ordering::Relaxed);
                let _ = heartbeat_store
                    .heartbeat_worker(&heartbeat_wid, active)
                    .await;
            }
        });

        // 2. Main Worker Processing Loop
        let lease_duration = Duration::from_secs(self.config.task_lease_seconds.max(30));

        while !cancel_token.is_cancelled() {
            let permit = match semaphore.clone().acquire_owned().await {
                Ok(p) => p,
                Err(_) => break,
            };

            // Attempt to reserve task from queue
            match self
                .queue
                .reserve(&self.worker_id, self.worker_type, lease_duration)
                .await
            {
                Ok(Some(lease)) => {
                    self.active_tasks.fetch_add(1, Ordering::Relaxed);
                    let worker_self = self.clone_worker_context();
                    let cancel = cancel_token.clone();

                    tokio::spawn(async move {
                        let _permit = permit;
                        worker_self.process_lease(lease, cancel).await;
                        worker_self.active_tasks.fetch_sub(1, Ordering::Relaxed);
                    });
                }
                Ok(None) => {
                    drop(permit);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => {
                    drop(permit);
                    warn!("Queue reserve error: {e}");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }

        info!("Worker [{}] shutting down gracefully...", self.worker_id);
    }

    fn clone_worker_context(&self) -> WorkerContext {
        WorkerContext {
            worker_id: self.worker_id.clone(),
            config: self.config.clone(),
            scraper: self.scraper.clone(),
            store: self.store.clone(),
            queue: self.queue.clone(),
            results: self.results.clone(),
            frontier: self.frontier.clone(),
            limiter: self.limiter.clone(),
            active_tasks: self.active_tasks.clone(),
        }
    }
}

#[derive(Clone)]
struct WorkerContext {
    worker_id: String,
    config: Arc<Config>,
    scraper: Arc<ScraperService>,
    store: Arc<dyn MetadataStore>,
    queue: Arc<dyn TaskQueue>,
    results: Arc<dyn ResultStore>,
    frontier: Arc<DistributedFrontier>,
    limiter: Arc<DistributedHostLimiter>,
    active_tasks: Arc<AtomicUsize>,
}

impl WorkerContext {
    async fn process_lease(&self, lease: TaskLease, _cancel: CancellationToken) {
        let task_id = lease.task_id.clone();
        info!(
            "Worker [{}] processing task [{}] of type {:?}",
            self.worker_id, task_id, lease.task_type
        );

        let _ = self
            .store
            .update_task_status(&task_id, TaskStatus::Running, Some(&lease), None)
            .await;

        let result = match lease.task_type {
            TaskType::HttpFetch | TaskType::BrowserRender => self.handle_fetch_task(&lease).await,
            TaskType::SitemapParse => self.handle_sitemap_task(&lease).await,
            _ => Ok(()),
        };

        match result {
            Ok(()) => {
                let _ = self.queue.ack(lease).await;
                let _ = self
                    .store
                    .update_task_status(&task_id, TaskStatus::Completed, None, None)
                    .await;
            }
            Err(err_msg) => {
                let failure = TaskFailure {
                    error_message: err_msg.clone(),
                    is_retryable: true,
                };
                let _ = self.queue.fail(lease, failure).await;
                let _ = self
                    .store
                    .update_task_status(&task_id, TaskStatus::Failed, None, Some(err_msg))
                    .await;
            }
        }
    }

    async fn handle_fetch_task(&self, lease: &TaskLease) -> Result<(), String> {
        let url_str = lease
            .payload
            .get("url")
            .and_then(|u| u.as_str())
            .ok_or_else(|| "Missing 'url' in task payload".to_string())?;

        let parsed_url = Url::parse(url_str).map_err(|e| e.to_string())?;
        let host = parsed_url.host_str().unwrap_or("default");

        // 1. Acquire host rate limit permit
        let _permit = self
            .limiter
            .acquire(
                host,
                self.config.max_concurrent_fetches_per_host,
                Duration::from_millis(self.config.default_request_delay_ms),
            )
            .await;

        // 2. Perform Scrape
        let mut scrape_req = ScrapeRequest::new(url_str);
        scrape_req.formats = Some(vec![
            OutputFormat::Markdown,
            OutputFormat::Html,
            OutputFormat::Links,
        ]);

        let doc = self
            .scraper
            .scrape(scrape_req)
            .await
            .map_err(|e| e.to_string())?;

        // 3. Store result in ResultStore
        let doc_bytes = serde_json::to_vec(&doc).map_err(|e| e.to_string())?;
        let url_hash = DistributedFrontier::compute_hash(url_str);
        let _location = self
            .results
            .put(
                &lease.job_id,
                &format!("pages/{url_hash}.json"),
                doc_bytes,
                "application/json",
            )
            .await
            .map_err(|e| e.to_string())?;

        // 4. Extract links & Enqueue new crawl tasks if within limits
        let current_depth = lease
            .payload
            .get("depth")
            .and_then(|d| d.as_u64())
            .unwrap_or(0) as u32;

        if current_depth < self.config.default_max_depth {
            if let Some(links) = doc.links {
                for link in links.into_iter().take(50) {
                    if let Some(entry) = self.frontier.insert_discovered(
                        &lease.job_id,
                        &link.url,
                        current_depth + 1,
                        true,
                    ) {
                        let q_task = QueuedTask::new(
                            &lease.job_id,
                            TaskType::HttpFetch,
                            serde_json::json!({
                                "url": entry.url,
                                "depth": current_depth + 1,
                            }),
                            TaskPriority::Normal,
                            5,
                        );
                        let _ = self.queue.enqueue(q_task).await;
                    }
                }
            }
        }

        // Emit PageCompleted event
        let status_code = doc.http.status;
        let event = SystemEventRecord {
            event_id: Uuid::new_v4().to_string(),
            job_id: lease.job_id.clone(),
            event_type: SystemEventType::PageCompleted,
            timestamp: Utc::now().timestamp() as u64,
            data: serde_json::json!({
                "url": url_str,
                "status_code": status_code,
            }),
        };
        let _ = self.store.store_event(event).await;

        Ok(())
    }

    async fn handle_sitemap_task(&self, lease: &TaskLease) -> Result<(), String> {
        let url_str = lease
            .payload
            .get("url")
            .and_then(|u| u.as_str())
            .ok_or_else(|| "Missing 'url' in payload".to_string())?;

        let sitemap_url = format!("{}/sitemap.xml", url_str.trim_end_matches('/'));
        let req = ScrapeRequest::new(sitemap_url);

        if let Ok(res) = self.scraper.scrape(req).await {
            if let Some(raw_xml) = res.html {
                if let Ok(crate::discovery::sitemap::ParsedSitemapContent::Urls(urls)) =
                    crate::discovery::sitemap::StreamingSitemapParser::parse_xml_str(
                        &raw_xml,
                        self.config.max_sitemap_urls,
                    )
                {
                    for u in urls.into_iter().take(self.config.max_map_urls) {
                        let loc_str = u.loc.to_string();
                        if let Some(entry) =
                            self.frontier
                                .insert_discovered(&lease.job_id, &loc_str, 1, true)
                        {
                            let q_task = QueuedTask::new(
                                &lease.job_id,
                                TaskType::HttpFetch,
                                serde_json::json!({
                                    "url": entry.url,
                                    "depth": 1,
                                }),
                                TaskPriority::Normal,
                                5,
                            );
                            let _ = self.queue.enqueue(q_task).await;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
