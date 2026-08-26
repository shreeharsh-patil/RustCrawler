use crate::render::models::NetworkResponse;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Manages bounded capture of XHR / Fetch JSON responses during page execution.
#[derive(Debug, Clone)]
pub struct NetworkCollector {
    responses: Arc<Mutex<Vec<NetworkResponse>>>,
    total_bytes: Arc<AtomicUsize>,
    max_responses: usize,
    max_response_bytes: usize,
    max_total_bytes: usize,
}

impl NetworkCollector {
    pub fn new(max_responses: usize, max_response_bytes: usize, max_total_bytes: usize) -> Self {
        Self {
            responses: Arc::new(Mutex::new(Vec::new())),
            total_bytes: Arc::new(AtomicUsize::new(0)),
            max_responses,
            max_response_bytes,
            max_total_bytes,
        }
    }

    pub async fn add_response(
        &self,
        url: String,
        method: String,
        status: u16,
        content_type: String,
        body_text: Option<String>,
    ) {
        // Filter out irrelevant media/style types
        let lower_ct = content_type.to_lowercase();
        if !lower_ct.contains("json")
            && !lower_ct.contains("text/plain")
            && !lower_ct.contains("javascript")
        {
            return;
        }

        let mut resps = self.responses.lock().await;
        if resps.len() >= self.max_responses {
            return;
        }

        let (parsed_body, raw_body, body_len) = if let Some(raw) = body_text {
            let len = raw.len().min(self.max_response_bytes);
            let truncated_str = if raw.len() > self.max_response_bytes {
                raw[..self.max_response_bytes].to_string()
            } else {
                raw
            };

            let json_val = if lower_ct.contains("json") {
                serde_json::from_str::<serde_json::Value>(&truncated_str).ok()
            } else {
                None
            };

            (json_val, Some(truncated_str), len)
        } else {
            (None, None, 0)
        };

        let current_total = self.total_bytes.load(Ordering::Relaxed);
        if current_total + body_len > self.max_total_bytes {
            return;
        }
        self.total_bytes.fetch_add(body_len, Ordering::Relaxed);

        resps.push(NetworkResponse {
            url,
            method,
            status,
            content_type,
            body: parsed_body,
            raw_body,
        });
    }

    pub async fn collect_all(&self) -> Vec<NetworkResponse> {
        let resps = self.responses.lock().await;
        resps.clone()
    }
}
