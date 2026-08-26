use std::time::Duration;

/// Determines if an HTTP status code represents a temporary, retryable condition.
pub fn is_retryable_status(status_code: u16) -> bool {
    matches!(status_code, 408 | 425 | 429 | 500 | 502 | 503 | 504)
}

/// Calculates the retry delay using exponential backoff with jitter.
pub fn calculate_retry_delay(
    attempt: usize,
    retry_after_secs: Option<u64>,
    base_delay_ms: u64,
) -> Duration {
    if let Some(secs) = retry_after_secs {
        if secs > 0 && secs <= 30 {
            return Duration::from_secs(secs);
        }
    }

    let multiplier = 1u64 << attempt.min(5);
    let backoff_ms = base_delay_ms.saturating_mul(multiplier);

    // Simple deterministic jitter based on timestamp
    let jitter_ms = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_millis() as u64)
        % 100;

    Duration::from_millis(backoff_ms + jitter_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable_status() {
        assert!(is_retryable_status(408));
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(500));
        assert!(is_retryable_status(502));
        assert!(is_retryable_status(503));
        assert!(is_retryable_status(504));

        assert!(!is_retryable_status(200));
        assert!(!is_retryable_status(301));
        assert!(!is_retryable_status(400));
        assert!(!is_retryable_status(401));
        assert!(!is_retryable_status(403));
        assert!(!is_retryable_status(404));
        assert!(!is_retryable_status(410));
    }

    #[test]
    fn test_calculate_retry_delay() {
        let d0 = calculate_retry_delay(0, None, 500);
        assert!(d0 >= Duration::from_millis(500) && d0 <= Duration::from_millis(600));

        let d1 = calculate_retry_delay(1, None, 500);
        assert!(d1 >= Duration::from_millis(1000) && d1 <= Duration::from_millis(1100));

        let d_retry_after = calculate_retry_delay(0, Some(5), 500);
        assert_eq!(d_retry_after, Duration::from_secs(5));
    }
}
