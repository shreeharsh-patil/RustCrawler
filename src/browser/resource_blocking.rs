/// Known tracking and analytics domains.
const TRACKER_DOMAINS: &[&str] = &[
    "google-analytics.com",
    "googletagmanager.com",
    "doubleclick.net",
    "googleadservices.com",
    "facebook.net",
    "connect.facebook.net",
    "clarity.ms",
    "hotjar.com",
    "segment.io",
    "segment.com",
    "mixpanel.com",
    "amplitude.com",
    "criteo.net",
    "adroll.com",
    "scorecardresearch.com",
    "quantserve.com",
    "matomo.cloud",
    "sentry.io",
    "browser.sentry-cdn.com",
];

/// Checks if a request URL matches known analytics/tracking domains.
pub fn is_tracker_url(url_str: &str) -> bool {
    let lower = url_str.to_lowercase();
    for domain in TRACKER_DOMAINS {
        if lower.contains(domain) {
            return true;
        }
    }
    false
}

/// Checks if a resource type should be blocked according to block_resources configuration.
pub fn is_resource_type_blocked(resource_type_str: &str, block_list: &[String]) -> bool {
    let lower_type = resource_type_str.to_lowercase();
    for blocked in block_list {
        let b = blocked.to_lowercase();
        if b == "font" && (lower_type == "font" || lower_type.contains("font")) {
            return true;
        }
        if b == "media" && (lower_type == "media" || lower_type == "audio" || lower_type == "video")
        {
            return true;
        }
        if b == "image" && lower_type == "image" {
            return true;
        }
        if b == "stylesheet" && lower_type == "stylesheet" {
            return true;
        }
    }
    false
}
