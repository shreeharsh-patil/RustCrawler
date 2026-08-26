use crate::error::CrawlerError;
use crate::render::models::WaitStrategy;
use chromiumoxide::Page;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::debug;

/// Executes a wait strategy bounded by the total timeout.
pub async fn execute_wait_strategy(
    page: &Page,
    strategy: &WaitStrategy,
    timeout: Duration,
    quiet_period: Duration,
) -> Result<(), CrawlerError> {
    let start = Instant::now();

    match strategy {
        WaitStrategy::Timeout(dur) => {
            let actual = (*dur).min(timeout);
            debug!("Waiting for fixed timeout of {}ms", actual.as_millis());
            sleep(actual).await;
            Ok(())
        }

        WaitStrategy::DomContentLoaded => {
            debug!("Waiting for DOMContentLoaded readyState");
            while start.elapsed() < timeout {
                if let Ok(ready_state) = page.evaluate("document.readyState").await {
                    if let Some(state_str) = ready_state.value().and_then(|v| v.as_str()) {
                        if state_str == "interactive" || state_str == "complete" {
                            // Short stabilization delay
                            sleep(Duration::from_millis(150)).await;
                            return Ok(());
                        }
                    }
                }
                sleep(Duration::from_millis(100)).await;
            }
            Ok(())
        }

        WaitStrategy::Load => {
            debug!("Waiting for document.readyState === complete");
            while start.elapsed() < timeout {
                if let Ok(ready_state) = page.evaluate("document.readyState").await {
                    if let Some(state_str) = ready_state.value().and_then(|v| v.as_str()) {
                        if state_str == "complete" {
                            sleep(Duration::from_millis(200)).await;
                            return Ok(());
                        }
                    }
                }
                sleep(Duration::from_millis(100)).await;
            }
            Ok(())
        }

        WaitStrategy::Selector(selector) => {
            debug!("Waiting for selector: {selector}");
            while start.elapsed() < timeout {
                if page.find_element(selector.as_str()).await.is_ok() {
                    return Ok(());
                }
                sleep(Duration::from_millis(100)).await;
            }
            Err(CrawlerError::SelectorNotFound(format!(
                "Timed out waiting for selector '{selector}' after {}ms",
                timeout.as_millis()
            )))
        }

        WaitStrategy::NetworkIdle => {
            debug!(
                "Waiting for NetworkIdle with quiet period {}ms",
                quiet_period.as_millis()
            );
            // 1. First ensure document is loaded
            while start.elapsed() < timeout {
                if let Ok(ready_state) = page.evaluate("document.readyState").await {
                    if let Some(state_str) = ready_state.value().and_then(|v| v.as_str()) {
                        if state_str == "complete" || state_str == "interactive" {
                            break;
                        }
                    }
                }
                sleep(Duration::from_millis(50)).await;
            }

            // 2. Wait for network quiet period
            sleep(quiet_period).await;
            Ok(())
        }
    }
}
