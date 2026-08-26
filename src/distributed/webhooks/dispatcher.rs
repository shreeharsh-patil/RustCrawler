use crate::distributed::models::{
    SystemEventRecord, WebhookDeliveryRecord, WebhookDeliveryStatus, WebhookEndpoint,
};
use crate::fetch::validation::validate_url_host_ssrf_preflight;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::Duration;
use tracing::warn;
use url::Url;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

pub struct WebhookDispatcher {
    client: reqwest::Client,
    signing_secret: Option<String>,
}

impl WebhookDispatcher {
    pub fn new(signing_secret: Option<String>, timeout: Duration) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();

        Self {
            client,
            signing_secret,
        }
    }

    pub fn compute_signature(secret: &str, timestamp: u64, payload: &str) -> String {
        let mut mac =
            HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
        let to_sign = format!("{timestamp}.{payload}");
        mac.update(to_sign.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    pub async fn dispatch(
        &self,
        endpoint: &WebhookEndpoint,
        event: &SystemEventRecord,
    ) -> Result<WebhookDeliveryRecord, String> {
        let now = Utc::now().timestamp() as u64;
        let delivery_id = Uuid::new_v4().to_string();

        // 1. SSRF Safety Verification
        let parsed_url =
            Url::parse(&endpoint.url).map_err(|e| format!("Invalid webhook URL: {e}"))?;

        if let Err(e) = validate_url_host_ssrf_preflight(&parsed_url, false) {
            warn!("Webhook delivery blocked by SSRF validator: {e}");
            return Ok(WebhookDeliveryRecord {
                delivery_id,
                event_id: event.event_id.clone(),
                job_id: event.job_id.clone(),
                endpoint_url: endpoint.url.clone(),
                event_type: event.event_type.clone(),
                status: WebhookDeliveryStatus::Failed,
                attempt: 1,
                response_status: None,
                error: Some(format!("SSRF Blocked: {e}")),
                next_retry_at: None,
                created_at: now,
            });
        }

        // 2. Prepare payload & headers
        let payload_json =
            serde_json::to_string(event).map_err(|e| format!("Failed to serialize event: {e}"))?;

        let mut req = self
            .client
            .post(&endpoint.url)
            .header("Content-Type", "application/json")
            .header("X-RustCrawler-Event", format!("{:?}", event.event_type))
            .header("X-RustCrawler-Timestamp", now.to_string())
            .header("X-RustCrawler-Delivery", &delivery_id);

        let secret_to_use = endpoint
            .secret
            .as_deref()
            .or(self.signing_secret.as_deref());
        if let Some(secret) = secret_to_use {
            let sig = Self::compute_signature(secret, now, &payload_json);
            req = req.header("X-RustCrawler-Signature", format!("sha256={sig}"));
        }

        // 3. Send HTTP POST
        match req.body(payload_json).send().await {
            Ok(resp) => {
                let status_code = resp.status().as_u16();
                let success = resp.status().is_success();

                Ok(WebhookDeliveryRecord {
                    delivery_id,
                    event_id: event.event_id.clone(),
                    job_id: event.job_id.clone(),
                    endpoint_url: endpoint.url.clone(),
                    event_type: event.event_type.clone(),
                    status: if success {
                        WebhookDeliveryStatus::Delivered
                    } else {
                        WebhookDeliveryStatus::Retrying
                    },
                    attempt: 1,
                    response_status: Some(status_code),
                    error: if success {
                        None
                    } else {
                        Some(format!("HTTP {status_code}"))
                    },
                    next_retry_at: if success { None } else { Some(now + 60) },
                    created_at: now,
                })
            }
            Err(err) => Ok(WebhookDeliveryRecord {
                delivery_id,
                event_id: event.event_id.clone(),
                job_id: event.job_id.clone(),
                endpoint_url: endpoint.url.clone(),
                event_type: event.event_type.clone(),
                status: WebhookDeliveryStatus::Retrying,
                attempt: 1,
                response_status: None,
                error: Some(err.to_string()),
                next_retry_at: Some(now + 60),
                created_at: now,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributed::models::SystemEventType;

    #[test]
    fn test_webhook_hmac_sha256_signature() {
        let secret = "my_super_secret_webhook_key";
        let timestamp = 1724628000;
        let payload = r#"{"event_id":"evt_1","event_type":"job_completed"}"#;

        let sig1 = WebhookDispatcher::compute_signature(secret, timestamp, payload);
        let sig2 = WebhookDispatcher::compute_signature(secret, timestamp, payload);

        assert_eq!(sig1, sig2, "HMAC signatures must be deterministic");
        assert_eq!(sig1.len(), 64, "SHA256 hex signature must be 64 characters");

        let sig_tampered =
            WebhookDispatcher::compute_signature(secret, timestamp, r#"{"tampered":true}"#);
        assert_ne!(sig1, sig_tampered);
    }

    #[tokio::test]
    async fn test_webhook_ssrf_protection_blocks_localhost_and_private_ips() {
        let dispatcher = WebhookDispatcher::new(Some("secret".to_string()), Duration::from_secs(5));

        let localhost_endpoint = WebhookEndpoint {
            id: "ep_1".to_string(),
            url: "http://127.0.0.1:8080/hook".to_string(),
            secret: None,
            events: vec![SystemEventType::JobCompleted],
            enabled: true,
        };

        let event = SystemEventRecord {
            event_id: "evt_1".to_string(),
            job_id: "job_1".to_string(),
            event_type: SystemEventType::JobCompleted,
            timestamp: 1724628000,
            data: serde_json::json!({}),
        };

        let delivery = dispatcher
            .dispatch(&localhost_endpoint, &event)
            .await
            .unwrap();
        assert_eq!(
            delivery.status,
            WebhookDeliveryStatus::Failed,
            "Localhost webhook destination must be blocked by SSRF protections"
        );
        assert!(delivery.error.unwrap().contains("SSRF Blocked"));
    }
}
