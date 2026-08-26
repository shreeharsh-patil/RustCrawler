use crate::config::Config;
use crate::error::CrawlerError;
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Locates a suitable Chromium/Chrome/Edge executable on the host system.
pub fn find_browser_executable(custom_path: Option<&str>) -> Result<PathBuf, CrawlerError> {
    // 1. Check custom path if provided
    if let Some(p) = custom_path {
        let path = PathBuf::from(p);
        if path.exists() {
            return Ok(path);
        } else {
            return Err(CrawlerError::BrowserNotFound(format!(
                "Specified browser executable not found at: {p}"
            )));
        }
    }

    // 2. Check standard system locations
    #[cfg(target_os = "windows")]
    {
        let standard_windows_paths = [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Users\Default\AppData\Local\Google\Chrome\Application\chrome.exe",
        ];

        for path_str in &standard_windows_paths {
            let path = Path::new(path_str);
            if path.exists() {
                debug!("Detected browser executable at: {:?}", path);
                return Ok(path.to_path_buf());
            }
        }

        if let Ok(which_path) = which::which("chrome.exe") {
            return Ok(which_path);
        }
        if let Ok(which_path) = which::which("msedge.exe") {
            return Ok(which_path);
        }
    }

    #[cfg(target_os = "macos")]
    {
        let standard_mac_paths = [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ];

        for path_str in &standard_mac_paths {
            let path = Path::new(path_str);
            if path.exists() {
                return Ok(path.to_path_buf());
            }
        }

        if let Ok(which_path) = which::which("google-chrome") {
            return Ok(which_path);
        }
        if let Ok(which_path) = which::which("chromium") {
            return Ok(which_path);
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let standard_linux_binaries = [
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
            "chrome",
            "microsoft-edge",
        ];

        for bin in &standard_linux_binaries {
            if let Ok(which_path) = which::which(bin) {
                return Ok(which_path);
            }
        }

        let standard_linux_paths = [
            "/usr/bin/google-chrome",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/snap/bin/chromium",
        ];

        for path_str in &standard_linux_paths {
            let path = Path::new(path_str);
            if path.exists() {
                return Ok(path.to_path_buf());
            }
        }
    }

    Err(CrawlerError::BrowserNotFound(
        "No Chrome, Chromium, or Edge executable detected. Please install Chromium or set BROWSER_EXECUTABLE_PATH.".to_string(),
    ))
}

/// Launches a new long-lived Chromium process with isolated data directory and optimal server flags.
pub async fn launch_browser(
    config: &Config,
) -> Result<(Browser, tokio::task::JoinHandle<()>), CrawlerError> {
    if !config.browser_enabled {
        return Err(CrawlerError::BrowserDisabled);
    }

    let exec_path = find_browser_executable(config.browser_executable_path.as_deref())?;
    info!("Launching Chromium from: {:?}", exec_path);

    let temp_data_dir = std::env::temp_dir().join(format!("rustcrawl_chrome_{}", Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&temp_data_dir);
    let user_data_arg = format!("--user-data-dir={}", temp_data_dir.display());

    let mut builder = BrowserConfig::builder();
    builder = builder
        .chrome_executable(exec_path)
        .arg(user_data_arg)
        .arg(format!("--user-agent={}", config.user_agent))
        .arg("--no-sandbox")
        .arg("--disable-dev-shm-usage")
        .arg("--disable-gpu")
        .arg("--disable-background-networking")
        .arg("--disable-default-apps")
        .arg("--disable-sync")
        .arg("--no-first-run")
        .arg("--mute-audio")
        .arg("--disable-features=Translate,InterestFeedContentSuggestions")
        .arg("--disable-client-side-phishing-detection");

    if !config.browser_headless {
        builder = builder.with_head();
    }

    let browser_cfg = builder.build().map_err(|e| {
        CrawlerError::BrowserLaunchFailed(format!("Failed to build browser config: {e}"))
    })?;

    let (browser, mut handler) = Browser::launch(browser_cfg).await.map_err(|e| {
        CrawlerError::BrowserLaunchFailed(format!("Failed to spawn browser process: {e}"))
    })?;

    let handle = tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            if let Err(err) = event {
                warn!("Chromium event handler warning: {err}");
            }
        }
        debug!("Chromium event handler stream terminated");
    });

    Ok((browser, handle))
}
