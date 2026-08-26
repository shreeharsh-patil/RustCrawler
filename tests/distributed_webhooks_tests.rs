use rustcrawl::distributed::models::{
    SystemEventRecord, SystemEventType, WebhookDeliveryStatus, WebhookEndpoint,
};
use rustcrawl::distributed::webhooks::dispatcher::WebhookDispatcher;
use std::time::Duration;

#[test]
fn test_webhook_hmac_sha256_signature() {
    let secret = "my_super_secret_webhook_key";
    let timestamp = 1724628000;
    let payload = r#"{"event_id":"evt_1","event_type":"job_completed"}"#;

    let sig1 = WebhookDispatcher::compute_signature(secret, timestamp, payload);
    let sig2 = WebhookDispatcher::compute_signature(secret, timestamp, payload);

    assert_eq!(sig1, sig2, "HMAC signatures must be deterministic");
    assert_eq!(sig1.len(), 64, "SHA256 hex signature must be 64 characters");

    // Tampered payload produces different signature
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
