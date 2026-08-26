use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::time::Duration;
use url::Url;

/// Rendering mode requested by user or configured as default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RenderMode {
    #[default]
    Auto,
    Http,
    Browser,
}

impl std::str::FromStr for RenderMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(RenderMode::Auto),
            "http" => Ok(RenderMode::Http),
            "browser" | "chrome" | "chromium" => Ok(RenderMode::Browser),
            _ => Err(format!(
                "Unknown render mode: '{s}'. Expected 'auto', 'http', or 'browser'"
            )),
        }
    }
}

/// Strategy for determining when page rendering is complete.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum WaitStrategy {
    Load,
    #[default]
    DomContentLoaded,
    NetworkIdle,
    Selector(String),
    Timeout(Duration),
}

impl Serialize for WaitStrategy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Load => serializer.serialize_str("load"),
            Self::DomContentLoaded => serializer.serialize_str("domcontentloaded"),
            Self::NetworkIdle => serializer.serialize_str("networkidle"),
            Self::Selector(sel) => {
                #[derive(Serialize)]
                struct SelObj<'a> {
                    selector: &'a str,
                }
                SelObj { selector: sel }.serialize(serializer)
            }
            Self::Timeout(dur) => {
                #[derive(Serialize)]
                struct TimeoutObj {
                    timeout_ms: u64,
                }
                TimeoutObj {
                    timeout_ms: dur.as_millis() as u64,
                }
                .serialize(serializer)
            }
        }
    }
}

impl<'de> Deserialize<'de> for WaitStrategy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct WaitStrategyVisitor;

        impl<'de> Visitor<'de> for WaitStrategyVisitor {
            type Value = WaitStrategy;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string ('load', 'domcontentloaded', 'networkidle') or an object with 'selector' or 'timeout_ms'")
            }

            fn visit_str<E>(self, value: &str) -> Result<WaitStrategy, E>
            where
                E: de::Error,
            {
                match value.to_lowercase().as_str() {
                    "load" => Ok(WaitStrategy::Load),
                    "domcontentloaded" | "dom_content_loaded" => Ok(WaitStrategy::DomContentLoaded),
                    "networkidle" | "network_idle" | "idle" => Ok(WaitStrategy::NetworkIdle),
                    _ => {
                        if let Some(stripped) = value.strip_prefix("selector:") {
                            Ok(WaitStrategy::Selector(stripped.to_string()))
                        } else if let Some(stripped) = value.strip_prefix("timeout:") {
                            if let Ok(ms) = stripped.parse::<u64>() {
                                Ok(WaitStrategy::Timeout(Duration::from_millis(ms)))
                            } else {
                                Err(de::Error::custom(format!(
                                    "Invalid timeout string: {value}"
                                )))
                            }
                        } else {
                            Err(de::Error::custom(format!(
                                "Unknown wait_for strategy string: '{value}'"
                            )))
                        }
                    }
                }
            }

            fn visit_map<M>(self, mut map: M) -> Result<WaitStrategy, M::Error>
            where
                M: de::MapAccess<'de>,
            {
                let mut selector: Option<String> = None;
                let mut timeout_ms: Option<u64> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "selector" => {
                            selector = Some(map.next_value()?);
                        }
                        "timeout_ms" | "timeout" | "milliseconds" => {
                            timeout_ms = Some(map.next_value()?);
                        }
                        _ => {
                            let _ = map.next_value::<de::IgnoredAny>()?;
                        }
                    }
                }

                if let Some(sel) = selector {
                    Ok(WaitStrategy::Selector(sel))
                } else if let Some(ms) = timeout_ms {
                    Ok(WaitStrategy::Timeout(Duration::from_millis(ms)))
                } else {
                    Err(de::Error::custom(
                        "Wait strategy object must contain either 'selector' or 'timeout_ms'",
                    ))
                }
            }
        }

        deserializer.deserialize_any(WaitStrategyVisitor)
    }
}

/// Typed browser interaction actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserAction {
    Wait {
        #[serde(default)]
        milliseconds: u64,
    },
    Click {
        selector: String,
    },
    Type {
        selector: String,
        text: String,
    },
    Press {
        key: String,
    },
    Scroll {
        #[serde(default)]
        x: Option<i64>,
        #[serde(default)]
        y: Option<i64>,
        #[serde(default)]
        direction: Option<String>,
        #[serde(default)]
        amount: Option<i64>,
    },
    ScrollTo {
        selector: String,
    },
    WaitForSelector {
        selector: String,
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
}

/// Captured network response payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkResponse {
    pub url: String,
    pub method: String,
    pub status: u16,
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_body: Option<String>,
}

/// Performance timing breakdown for browser rendering.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenderTimings {
    pub acquire_time_ms: u64,
    pub navigation_time_ms: u64,
    pub wait_time_ms: u64,
    pub action_time_ms: u64,
    pub total_render_time_ms: u64,
}

/// Request parameters for rendering a page via browser.
#[derive(Debug, Clone)]
pub struct RenderRequest {
    pub url: Url,
    pub wait_strategy: WaitStrategy,
    pub timeout: Duration,
    pub actions: Vec<BrowserAction>,
    pub auto_scroll: bool,
    pub capture_network: bool,
    pub block_trackers: bool,
    pub block_resources: Vec<String>,
    pub user_agent: String,
}

/// Output of a browser rendering operation.
#[derive(Debug, Clone)]
pub struct RenderResult {
    pub final_url: Url,
    pub html: String,
    pub status_code: Option<u16>,
    pub network_responses: Vec<NetworkResponse>,
    pub timings: RenderTimings,
}

/// Identifies which engine produced the scraped page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectedRenderer {
    Http,
    Browser,
}

impl SelectedRenderer {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Browser => "browser",
        }
    }
}

/// Explanatory reason for why a specific renderer was chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderReason {
    HttpContentSufficient,
    EmptySpaRoot,
    EmptyBodyOrMain,
    LowVisibleText,
    ScriptsDominated,
    EnableJavascriptMessage,
    HydrationMarkerPresent,
    PlaceholdersOnly,
    ForcedBrowserMode,
    ForcedHttpMode,
    BrowserFallbackFailed,
}

impl RenderReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HttpContentSufficient => "http_content_sufficient",
            Self::EmptySpaRoot => "empty_spa_root",
            Self::EmptyBodyOrMain => "empty_body_or_main",
            Self::LowVisibleText => "low_visible_text",
            Self::ScriptsDominated => "scripts_dominated",
            Self::EnableJavascriptMessage => "enable_javascript_message",
            Self::HydrationMarkerPresent => "hydration_marker_present",
            Self::PlaceholdersOnly => "placeholders_only",
            Self::ForcedBrowserMode => "forced_browser_mode",
            Self::ForcedHttpMode => "forced_http_mode",
            Self::BrowserFallbackFailed => "browser_fallback_failed",
        }
    }
}

/// Diagnostic metadata from the render decision engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderDiagnostics {
    pub visible_text_length: usize,
    pub script_count: usize,
    pub paragraph_count: usize,
    pub link_count: usize,
    pub framework_markers: Vec<String>,
    pub render_score: i32,
    pub render_threshold: i32,
    pub selected_renderer: SelectedRenderer,
    pub reasons: Vec<RenderReason>,
}
