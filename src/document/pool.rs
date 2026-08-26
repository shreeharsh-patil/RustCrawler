use crate::error::CrawlerError;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// Managed semaphore-bounded pool for offloading CPU-heavy parsing (PDF, DOCX, huge CSV) without blocking the async runtime.
#[derive(Clone)]
pub struct DocumentParserPool {
    semaphore: Arc<Semaphore>,
}

impl DocumentParserPool {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent.max(1))),
        }
    }

    /// Executes a CPU-bound parsing closure on a blocking thread pool bounded by concurrency limits and timeout.
    pub async fn execute_cpu_bound<F, R>(&self, timeout: Duration, f: F) -> Result<R, CrawlerError>
    where
        F: FnOnce() -> Result<R, CrawlerError> + Send + 'static,
        R: Send + 'static,
    {
        // 1. Acquire CPU permit
        let _permit =
            self.semaphore.acquire().await.map_err(|_| {
                CrawlerError::InternalError("Parser pool semaphore closed".to_string())
            })?;

        // 2. Run in blocking task bounded by timeout
        let blocking_task = tokio::task::spawn_blocking(f);

        match tokio::time::timeout(timeout, blocking_task).await {
            Ok(Ok(result)) => result,
            Ok(Err(join_err)) => Err(CrawlerError::InternalError(format!(
                "Parser worker task panicked: {join_err}"
            ))),
            Err(_) => Err(CrawlerError::RequestTimeout(format!(
                "Document parsing timed out after {}s",
                timeout.as_secs()
            ))),
        }
    }
}
