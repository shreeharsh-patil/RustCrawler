use crate::browser::chromium::launch_browser;
use crate::browser::page::render_page;
use crate::config::Config;
use crate::error::CrawlerError;
use crate::render::models::{RenderRequest, RenderResult};
use crate::render::PageRenderer;
use async_trait::async_trait;
use chromiumoxide::Browser;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tracing::{debug, info, warn};

type BrowserHandle = (Arc<Browser>, tokio::task::JoinHandle<()>);

/// Thread-safe browser manager providing pooled page execution and lazy Chromium lifecycle management.
#[derive(Clone)]
pub struct BrowserManager {
    config: Config,
    semaphore: Arc<Semaphore>,
    browser_cell: Arc<Mutex<Option<BrowserHandle>>>,
}

impl BrowserManager {
    pub fn new(config: Config) -> Self {
        let max_pages = config.max_browser_pages;
        Self {
            config,
            semaphore: Arc::new(Semaphore::new(max_pages)),
            browser_cell: Arc::new(Mutex::new(None)),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn is_enabled(&self) -> bool {
        self.config.browser_enabled
    }

    pub fn active_pages(&self) -> usize {
        self.config
            .max_browser_pages
            .saturating_sub(self.semaphore.available_permits())
    }

    pub fn max_pages(&self) -> usize {
        self.config.max_browser_pages
    }

    /// Retrieves an existing active browser instance or lazily spawns a new one.
    pub async fn get_or_launch(&self) -> Result<Arc<Browser>, CrawlerError> {
        if !self.config.browser_enabled {
            return Err(CrawlerError::BrowserDisabled);
        }

        let mut lock = self.browser_cell.lock().await;
        if let Some((ref browser, _)) = *lock {
            return Ok(browser.clone());
        }

        info!("Initializing and launching shared Chromium browser instance");
        let (browser, handle) = launch_browser(&self.config).await?;
        let browser_arc = Arc::new(browser);
        *lock = Some((browser_arc.clone(), handle));
        Ok(browser_arc)
    }

    /// Checks if the browser is currently running and healthy.
    pub async fn is_healthy(&self) -> bool {
        if !self.config.browser_enabled {
            return false;
        }
        let lock = self.browser_cell.lock().await;
        lock.is_some()
    }
}

#[async_trait]
impl PageRenderer for BrowserManager {
    async fn render(&self, request: RenderRequest) -> Result<RenderResult, CrawlerError> {
        if !self.config.browser_enabled {
            return Err(CrawlerError::BrowserDisabled);
        }

        let acquire_timeout = self.config.browser_acquire_timeout();
        let total_timeout = self.config.browser_total_timeout();

        debug!(
            "Acquiring browser page permit (available: {}/{}) for '{}'",
            self.semaphore.available_permits(),
            self.config.max_browser_pages,
            request.url
        );

        // 1. Acquire permit with timeout to provide backpressure
        let _permit = match tokio::time::timeout(acquire_timeout, self.semaphore.acquire()).await {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => {
                return Err(CrawlerError::BrowserUnavailable(
                    "Browser semaphore closed".to_string(),
                ));
            }
            Err(_) => {
                return Err(CrawlerError::BrowserPoolExhausted(
                    self.config.max_browser_pages,
                ));
            }
        };

        // 2. Obtain browser instance (lazy launch if necessary)
        let browser = self.get_or_launch().await?;

        // 3. Execute page render task bounded by total timeout
        let config_clone = self.config.clone();
        let render_task = render_page(&browser, request, &config_clone);

        match tokio::time::timeout(total_timeout, render_task).await {
            Ok(result) => result,
            Err(_) => {
                warn!(
                    "Total browser render timeout exceeded ({}s)",
                    total_timeout.as_secs()
                );
                Err(CrawlerError::BrowserTimeout(format!(
                    "Total page render timeout of {}s exceeded",
                    total_timeout.as_secs()
                )))
            }
        }
    }
}
