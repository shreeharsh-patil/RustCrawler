use crate::config::Config;
use crate::error::CrawlerError;
use crate::fetch::dns::resolve_and_validate_url;
use crate::fetch::response::FetchedDocument;
use crate::fetch::validation::validate_url_host_ssrf_preflight;
use crate::models::{ScrapeWarning, WarningCode};
use crate::utils::urls::{resolve_relative_url, validate_url_syntax};
use bytes::BytesMut;
use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, USER_AGENT};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Secure HTTP client with SSRF protection, size bounds, and streaming responses.
#[derive(Clone)]
pub struct HttpFetcher {
    client: reqwest::Client,
    config: Arc<Config>,
}

impl HttpFetcher {
    pub fn new(config: Arc<Config>) -> Result<Self, CrawlerError> {
        let mut default_headers = HeaderMap::new();
        default_headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&config.user_agent)
                .map_err(|e| CrawlerError::InternalError(format!("Invalid User-Agent: {e}")))?,
        );
        default_headers.insert(
            ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/json,application/xml,text/plain,application/pdf,*/*;q=0.8",
            ),
        );
        default_headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));

        let client = reqwest::Client::builder()
            .default_headers(default_headers)
            // Manual redirect handling for per-hop SSRF validation
            .redirect(reqwest::redirect::Policy::none())
            .timeout(config.request_timeout())
            .pool_idle_timeout(Some(Duration::from_secs(90)))
            .pool_max_idle_per_host(10)
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .build()
            .map_err(|e| {
                CrawlerError::InternalError(format!("Failed to build HTTP client: {e}"))
            })?;

        Ok(Self { client, config })
    }

    /// Fetches a URL while enforcing SSRF checks, redirect limits, and size bounds.
    pub async fn fetch(&self, raw_url: &str) -> Result<FetchedDocument, CrawlerError> {
        let start_time = Instant::now();
        let initial_url = validate_url_syntax(raw_url)?;
        let mut current_url = initial_url.clone();
        let mut redirect_count = 0;
        let mut warnings = Vec::new();
        let max_size = self.config.max_response_size_bytes();

        loop {
            // 1. SSRF preflight and DNS validation before each network hop
            validate_url_host_ssrf_preflight(&current_url, self.config.allow_private_networks)?;
            let _ =
                resolve_and_validate_url(&current_url, self.config.allow_private_networks).await?;

            // 2. Perform HTTP GET request
            let response = match self.client.get(current_url.clone()).send().await {
                Ok(resp) => resp,
                Err(err) => {
                    if err.is_timeout() {
                        return Err(CrawlerError::RequestTimeout(format!(
                            "Request to '{current_url}' timed out after {}s",
                            self.config.request_timeout_seconds
                        )));
                    } else if err.is_connect() {
                        return Err(CrawlerError::ConnectionFailed(format!(
                            "Failed to connect to '{current_url}': {err}"
                        )));
                    } else {
                        return Err(CrawlerError::ConnectionFailed(format!(
                            "HTTP request failed for '{current_url}': {err}"
                        )));
                    }
                }
            };

            let status = response.status();

            // 3. Handle redirects (301, 302, 303, 307, 308)
            if status.is_redirection() {
                redirect_count += 1;
                if redirect_count > self.config.max_redirects {
                    return Err(CrawlerError::TooManyRedirects(self.config.max_redirects));
                }

                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|h| h.to_str().ok())
                    .ok_or_else(|| {
                        CrawlerError::ConnectionFailed(
                            "Redirect response missing Location header".to_string(),
                        )
                    })?;

                let next_url = resolve_relative_url(&current_url, location).ok_or_else(|| {
                    CrawlerError::InvalidUrl(format!("Invalid redirect Location: '{location}'"))
                })?;

                // Scheme check on redirect target
                if next_url.scheme() != "http" && next_url.scheme() != "https" {
                    return Err(CrawlerError::UnsupportedScheme(
                        next_url.scheme().to_string(),
                    ));
                }

                current_url = next_url;
                continue;
            }

            // 4. Content-Type Header
            let content_type = response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("text/html; charset=utf-8")
                .to_string();

            // 5. Check Content-Length header if present
            if let Some(content_length) = response.content_length() {
                if content_length as usize > max_size {
                    return Err(CrawlerError::ResponseTooLarge(max_size));
                }
            }

            // 6. Stream response body with size limit enforcement
            let mut stream = response;
            let mut body_bytes = BytesMut::with_capacity(64 * 1024);

            while let Some(chunk) = stream
                .chunk()
                .await
                .map_err(|e| CrawlerError::ConnectionFailed(format!("Error reading stream: {e}")))?
            {
                if body_bytes.len() + chunk.len() > max_size {
                    return Err(CrawlerError::ResponseTooLarge(max_size));
                }
                body_bytes.extend_from_slice(&chunk);
            }

            let total_bytes = body_bytes.len();

            // 7. Charset decoding (for text/html)
            let (html_string, charset_warning) = decode_body(&body_bytes, &content_type);
            if let Some(w) = charset_warning {
                warnings.push(w);
            }

            let fetch_time_ms = start_time.elapsed().as_millis() as u64;

            return Ok(FetchedDocument {
                initial_url,
                final_url: current_url,
                status: status.as_u16(),
                content_type,
                content_length: total_bytes,
                body: body_bytes.to_vec(),
                html: html_string,
                fetch_time_ms,
                warnings,
            });
        }
    }
}

/// Decodes raw body bytes into a UTF-8 String using Content-Type charset or fallback detection.
fn decode_body(bytes: &[u8], content_type: &str) -> (String, Option<ScrapeWarning>) {
    // 1. Check charset parameter in Content-Type
    let mut detected_charset = None;
    for part in content_type.split(';') {
        let trimmed = part.trim();
        if let Some(rest) = trimmed.strip_prefix("charset=") {
            let charset_name = rest.trim_matches('"').trim_matches('\'').trim();
            if !charset_name.is_empty() {
                detected_charset = Some(charset_name);
                break;
            }
        }
    }

    if let Some(charset) = detected_charset {
        if let Some(encoding) = Encoding::for_label(charset.as_bytes()) {
            let (cow, had_errors) = encoding.decode_without_bom_handling(bytes);
            let warning = if had_errors {
                Some(ScrapeWarning::new(
                    WarningCode::CharsetFallback,
                    format!("Encountered malformed byte sequences while decoding as {charset}"),
                ))
            } else {
                None
            };
            return (cow.into_owned(), warning);
        }
    }

    // 2. Check BOM (Byte Order Mark)
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let (cow, _) = UTF_8.decode_without_bom_handling(&bytes[3..]);
        return (cow.into_owned(), None);
    }

    // 3. Default: try UTF-8, fallback to Windows-1252
    match std::str::from_utf8(bytes) {
        Ok(valid_utf8) => (valid_utf8.to_string(), None),
        Err(_) => {
            let (cow, _) = WINDOWS_1252.decode_without_bom_handling(bytes);
            (
                cow.into_owned(),
                Some(ScrapeWarning::new(
                    WarningCode::CharsetFallback,
                    "UTF-8 decoding failed; fell back to Windows-1252",
                )),
            )
        }
    }
}
