use crate::error::CrawlerError;
use crate::render::models::BrowserAction;
use chromiumoxide::Page;
use std::time::Duration;
use tokio::time::sleep;
use tracing::debug;

const MAX_SELECTOR_LEN: usize = 500;
const MAX_TEXT_LEN: usize = 5000;

/// Validates and executes a sequence of typed browser actions on a page.
pub async fn execute_browser_actions(
    page: &Page,
    actions: &[BrowserAction],
    max_action_wait: Duration,
    max_actions: usize,
) -> Result<(), CrawlerError> {
    if actions.len() > max_actions {
        return Err(CrawlerError::ActionFailed(
            "validation".to_string(),
            format!(
                "Too many actions: {} exceeds limit of {}",
                actions.len(),
                max_actions
            ),
        ));
    }

    for (idx, action) in actions.iter().enumerate() {
        debug!("Executing browser action [{idx}]: {action:?}");
        match action {
            BrowserAction::Wait { milliseconds } => {
                let dur = Duration::from_millis(*milliseconds).min(max_action_wait);
                sleep(dur).await;
            }

            BrowserAction::Click { selector } => {
                if selector.len() > MAX_SELECTOR_LEN {
                    return Err(CrawlerError::ActionFailed(
                        "click".to_string(),
                        "Selector exceeds maximum allowed length".to_string(),
                    ));
                }

                let escaped = selector.replace('\'', "\\'");
                let js = format!(
                    r#"(function() {{
                        const el = document.querySelector('{escaped}');
                        if (!el) return false;
                        el.click();
                        return true;
                    }})()"#
                );

                match page.evaluate(js).await {
                    Ok(val) => {
                        if let Some(clicked) = val.value().and_then(|v| v.as_bool()) {
                            if !clicked {
                                return Err(CrawlerError::SelectorNotFound(format!(
                                    "Selector '{selector}' not found for click"
                                )));
                            }
                        }
                    }
                    Err(e) => {
                        return Err(CrawlerError::ActionFailed(
                            "click".to_string(),
                            e.to_string(),
                        ));
                    }
                }
            }

            BrowserAction::Type { selector, text } => {
                if selector.len() > MAX_SELECTOR_LEN || text.len() > MAX_TEXT_LEN {
                    return Err(CrawlerError::ActionFailed(
                        "type".to_string(),
                        "Selector or text exceeds maximum allowed length".to_string(),
                    ));
                }

                let escaped_sel = selector.replace('\'', "\\'");
                let escaped_text = text.replace('\'', "\\'").replace('\n', "\\n");
                let js = format!(
                    r#"(function() {{
                        const el = document.querySelector('{escaped_sel}');
                        if (!el) return false;
                        el.focus();
                        el.value = '{escaped_text}';
                        el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                        return true;
                    }})()"#
                );

                match page.evaluate(js).await {
                    Ok(val) => {
                        if let Some(typed) = val.value().and_then(|v| v.as_bool()) {
                            if !typed {
                                return Err(CrawlerError::SelectorNotFound(format!(
                                    "Selector '{selector}' not found for type"
                                )));
                            }
                        }
                    }
                    Err(e) => {
                        return Err(CrawlerError::ActionFailed(
                            "type".to_string(),
                            e.to_string(),
                        ));
                    }
                }
            }

            BrowserAction::Press { key } => {
                let escaped_key = key.replace('\\', "\\\\").replace('\'', "\\'");
                let js = format!(
                    "window.dispatchEvent(new KeyboardEvent('keydown', {{ key: '{escaped_key}' }}));"
                );
                page.evaluate(js)
                    .await
                    .map_err(|e| CrawlerError::ActionFailed("press".to_string(), e.to_string()))?;
            }

            BrowserAction::Scroll {
                x,
                y,
                direction,
                amount,
            } => {
                let scroll_y = if let Some(amt) = amount {
                    if direction.as_deref().unwrap_or("down") == "up" {
                        -amt
                    } else {
                        *amt
                    }
                } else {
                    y.unwrap_or(500)
                };
                let scroll_x = x.unwrap_or(0);

                let js = format!("window.scrollBy({scroll_x}, {scroll_y});");
                page.evaluate(js)
                    .await
                    .map_err(|e| CrawlerError::ActionFailed("scroll".to_string(), e.to_string()))?;
            }

            BrowserAction::ScrollTo { selector } => {
                if selector.len() > MAX_SELECTOR_LEN {
                    return Err(CrawlerError::ActionFailed(
                        "scroll_to".to_string(),
                        "Selector exceeds maximum allowed length".to_string(),
                    ));
                }

                let escaped_sel = selector.replace('\'', "\\'");
                let js = format!(
                    "document.querySelector('{escaped_sel}')?.scrollIntoView({{ behavior: 'smooth', block: 'center' }});"
                );
                page.evaluate(js).await.map_err(|e| {
                    CrawlerError::ActionFailed("scroll_to".to_string(), e.to_string())
                })?;
            }

            BrowserAction::WaitForSelector {
                selector,
                timeout_ms,
            } => {
                if selector.len() > MAX_SELECTOR_LEN {
                    return Err(CrawlerError::ActionFailed(
                        "wait_for_selector".to_string(),
                        "Selector exceeds maximum allowed length".to_string(),
                    ));
                }

                let wait_timeout = timeout_ms
                    .map(Duration::from_millis)
                    .unwrap_or(max_action_wait)
                    .min(max_action_wait);

                let start = std::time::Instant::now();
                loop {
                    if page.find_element(selector.as_str()).await.is_ok() {
                        break;
                    }
                    if start.elapsed() >= wait_timeout {
                        return Err(CrawlerError::SelectorNotFound(format!(
                            "Timed out waiting for selector '{selector}' after {}ms",
                            wait_timeout.as_millis()
                        )));
                    }
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    Ok(())
}
