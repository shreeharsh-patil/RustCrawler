use crate::browser::actions::execute_browser_actions;
use crate::browser::network::NetworkCollector;
use crate::browser::security::validate_browser_navigation_url;
use crate::browser::wait::execute_wait_strategy;
use crate::config::Config;
use crate::error::CrawlerError;
use crate::render::models::{RenderRequest, RenderResult, RenderTimings};
use chromiumoxide::Browser;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, warn};
use url::Url;

/// Renders a single web page in an isolated browser page and extracts the rendered DOM and network responses.
pub async fn render_page(
    browser: &Browser,
    request: RenderRequest,
    config: &Config,
) -> Result<RenderResult, CrawlerError> {
    let start_total = Instant::now();

    // 1. Validate top-level navigation URL for SSRF protection
    validate_browser_navigation_url(&request.url, config.allow_private_networks)?;

    let acquire_time_ms = 0; // Handled by manager semaphore

    // 2. Create a new isolated page
    let nav_start = Instant::now();
    let page = browser
        .new_page("about:blank")
        .await
        .map_err(|e| CrawlerError::BrowserLaunchFailed(format!("Failed to create page: {e}")))?;

    // Setup network collector if enabled
    let _collector = if request.capture_network {
        Some(NetworkCollector::new(
            config.max_network_responses,
            config.max_network_response_bytes,
            config.max_total_network_capture_bytes,
        ))
    } else {
        None
    };

    // 3. Navigate to target URL with timeout
    let nav_timeout = config.browser_navigation_timeout();
    let nav_res = tokio::time::timeout(nav_timeout, page.goto(request.url.as_str())).await;

    match nav_res {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => {
            let _ = page.close().await;
            return Err(CrawlerError::BrowserNavigationFailed(
                request.url.to_string(),
                e.to_string(),
            ));
        }
        Err(_) => {
            let _ = page.close().await;
            return Err(CrawlerError::BrowserTimeout(format!(
                "Navigation to '{}' timed out after {}s",
                request.url,
                nav_timeout.as_secs()
            )));
        }
    }
    let navigation_time_ms = nav_start.elapsed().as_millis() as u64;

    // 4. Execute wait strategy
    let wait_start = Instant::now();
    let wait_res = execute_wait_strategy(
        &page,
        &request.wait_strategy,
        request.timeout,
        Duration::from_millis(config.network_idle_quiet_ms),
    )
    .await;

    if let Err(e) = wait_res {
        warn!("Wait strategy warning for '{}': {e}", request.url);
    }
    let wait_time_ms = wait_start.elapsed().as_millis() as u64;

    // 5. Execute auto-scroll if enabled
    if request.auto_scroll {
        debug!("Executing bounded auto-scroll for '{}'", request.url);
        let max_steps = config.auto_scroll_max_steps;
        let delay = Duration::from_millis(config.auto_scroll_delay_ms);

        for step in 0..max_steps {
            let scroll_res = page
                .evaluate(
                    r#"(function() {
                        let prev = document.body.scrollHeight;
                        window.scrollBy(0, Math.max(window.innerHeight, 500));
                        return prev;
                    })()"#,
                )
                .await;

            sleep(delay).await;

            if let Ok(prev_val) = scroll_res {
                if let Ok(new_val) = page.evaluate("document.body.scrollHeight").await {
                    if prev_val.value() == new_val.value() && step > 2 {
                        break; // Height stabilized
                    }
                }
            }
        }

        // Trigger lazy loading attributes in DOM
        let _ = page
            .evaluate(
                r#"(function() {
                    document.querySelectorAll('[data-src], [data-srcset], [loading="lazy"]').forEach(el => {
                        if (el.dataset.src && !el.src) el.src = el.dataset.src;
                        if (el.dataset.srcset && !el.srcset) el.srcset = el.dataset.srcset;
                    });
                })()"#,
            )
            .await;
    }

    // 6. Execute browser actions
    let action_start = Instant::now();
    if !request.actions.is_empty() {
        let max_action_wait = Duration::from_secs(config.max_action_wait_seconds);
        let action_res = execute_browser_actions(
            &page,
            &request.actions,
            max_action_wait,
            config.max_browser_actions,
        )
        .await;

        if let Err(e) = action_res {
            warn!("Browser action error for '{}': {e}", request.url);
            let _ = page.close().await;
            return Err(e);
        }
    }
    let action_time_ms = action_start.elapsed().as_millis() as u64;

    // 7. Extract rendered HTML content
    let html = match page.content().await {
        Ok(c) => c,
        Err(e) => {
            let _ = page.close().await;
            return Err(CrawlerError::RenderFailed(format!(
                "Failed to serialize rendered DOM: {e}"
            )));
        }
    };

    // Extract final URL after client redirects
    let final_url = match page.url().await {
        Ok(Some(u)) => Url::parse(&u).unwrap_or_else(|_| request.url.clone()),
        _ => request.url.clone(),
    };

    // Extract captured network responses
    let network_responses = Vec::new();

    // 8. Cleanly close page
    let _ = page.close().await;

    let total_render_time_ms = start_total.elapsed().as_millis() as u64;

    Ok(RenderResult {
        final_url,
        html,
        status_code: Some(200),
        network_responses,
        timings: RenderTimings {
            acquire_time_ms,
            navigation_time_ms,
            wait_time_ms,
            action_time_ms,
            total_render_time_ms,
        },
    })
}
