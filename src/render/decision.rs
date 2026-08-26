use crate::render::models::{RenderDiagnostics, RenderReason, SelectedRenderer};
use scraper::{Html, Selector};

/// Summary analysis of an HTTP scrape used to determine if JavaScript rendering is needed.
#[derive(Debug, Clone)]
pub struct HttpScrapeAnalysis {
    pub visible_text_length: usize,
    pub main_content_length: usize,
    pub script_count: usize,
    pub paragraph_count: usize,
    pub link_count: usize,
    pub has_empty_root_container: bool,
    pub has_enable_js_message: bool,
    pub has_placeholders_only: bool,
    pub framework_markers: Vec<String>,
}

impl HttpScrapeAnalysis {
    pub fn analyze(html_str: &str, main_content_text: &str) -> Self {
        let document = Html::parse_document(html_str);

        // Paragraph count
        let p_sel = Selector::parse("p").ok();
        let paragraph_count = p_sel.map(|s| document.select(&s).count()).unwrap_or(0);

        // Script tags count
        let script_sel = Selector::parse("script").ok();
        let script_count = script_sel.map(|s| document.select(&s).count()).unwrap_or(0);

        // Links count
        let a_sel = Selector::parse("a[href]").ok();
        let link_count = a_sel.map(|s| document.select(&s).count()).unwrap_or(0);

        // Visible text extraction: excludes <script>, <style>, <noscript>
        let body_sel = Selector::parse("body").ok();
        let mut visible_text = String::new();
        if let Some(body_elem) = body_sel.as_ref().and_then(|s| document.select(s).next()) {
            for node in body_elem.descendants() {
                if let Some(elem) = node.value().as_element() {
                    let name = elem.name();
                    if name == "script"
                        || name == "style"
                        || name == "noscript"
                        || name == "template"
                    {
                        continue;
                    }
                }
                if let Some(text) = node.value().as_text() {
                    let mut parent = node.parent();
                    let mut in_ignored = false;
                    while let Some(p) = parent {
                        if let Some(p_elem) = p.value().as_element() {
                            let p_name = p_elem.name();
                            if p_name == "script"
                                || p_name == "style"
                                || p_name == "noscript"
                                || p_name == "template"
                            {
                                in_ignored = true;
                                break;
                            }
                        }
                        parent = p.parent();
                    }
                    if !in_ignored {
                        visible_text.push_str(text);
                        visible_text.push(' ');
                    }
                }
            }
        }

        let visible_text_length = visible_text.trim().len();
        let main_content_length = main_content_text.trim().len();

        // Check for empty SPA root containers (#root, #app, #__next, #main-app)
        let mut has_empty_root_container = false;
        let root_selectors = [
            "#root",
            "#app",
            "#__next",
            "#main-content",
            "[data-reactroot]",
        ];
        for sel_str in root_selectors {
            if let Ok(sel) = Selector::parse(sel_str) {
                if let Some(elem) = document.select(&sel).next() {
                    let inner_text = elem.text().collect::<Vec<_>>().join(" ");
                    if inner_text.trim().len() < 80 {
                        has_empty_root_container = true;
                        break;
                    }
                }
            }
        }

        // Check for "enable javascript" messages
        let lower_html = html_str.to_lowercase();
        let has_enable_js_message = lower_html.contains("please enable javascript")
            || lower_html.contains("enable javascript to run")
            || lower_html.contains("you need to enable javascript")
            || lower_html.contains("javascript is required")
            || lower_html.contains("javascript is disabled");

        // Check for framework hydration markers
        let mut framework_markers = Vec::new();
        if lower_html.contains("__next_data__") || lower_html.contains("/_next/static/") {
            framework_markers.push("nextjs".to_string());
        }
        if lower_html.contains("data-reactroot") || lower_html.contains("react-dom") {
            framework_markers.push("react".to_string());
        }
        if lower_html.contains("data-vue-meta")
            || lower_html.contains("v-app")
            || lower_html.contains("__nuxt__")
        {
            framework_markers.push("vue_nuxt".to_string());
        }
        if lower_html.contains("ng-version") || lower_html.contains("ng-app") {
            framework_markers.push("angular".to_string());
        }
        if lower_html.contains("svelte-") {
            framework_markers.push("svelte".to_string());
        }

        // Check for loading skeletons / placeholder spinners with little real content
        let has_placeholders_only = (lower_html.contains("skeleton-loader")
            || lower_html.contains("loading-spinner")
            || lower_html.contains("shimmer-card"))
            && visible_text_length < 400;

        Self {
            visible_text_length,
            main_content_length,
            script_count,
            paragraph_count,
            link_count,
            has_empty_root_container,
            has_enable_js_message,
            has_placeholders_only,
            framework_markers,
        }
    }
}

/// Output of the decision engine.
#[derive(Debug, Clone)]
pub struct RenderDecision {
    pub renderer: SelectedRenderer,
    pub score: i32,
    pub reasons: Vec<RenderReason>,
    pub diagnostics: RenderDiagnostics,
}

/// Trait for smart JavaScript detection and render dispatching.
pub trait RenderDecisionEngine: Send + Sync {
    fn decide(&self, analysis: &HttpScrapeAnalysis) -> RenderDecision;
}

/// Default scoring engine for intelligent JavaScript detection.
#[derive(Debug, Clone)]
pub struct SmartDecisionEngine {
    pub threshold: i32,
}

impl SmartDecisionEngine {
    pub fn new(threshold: i32) -> Self {
        Self { threshold }
    }
}

impl Default for SmartDecisionEngine {
    fn default() -> Self {
        Self { threshold: 5 }
    }
}

impl RenderDecisionEngine for SmartDecisionEngine {
    fn decide(&self, analysis: &HttpScrapeAnalysis) -> RenderDecision {
        let mut score: i32 = 0;
        let mut reasons = Vec::new();

        // 1. Very low visible text content (+3)
        if analysis.visible_text_length < 200 {
            score += 3;
            reasons.push(RenderReason::LowVisibleText);
        }

        // 2. Empty body or main content (+3)
        if analysis.main_content_length == 0 || analysis.visible_text_length == 0 {
            score += 3;
            reasons.push(RenderReason::EmptyBodyOrMain);
        }

        // 3. Empty SPA root container (+3)
        if analysis.has_empty_root_container {
            score += 3;
            reasons.push(RenderReason::EmptySpaRoot);
        }

        // 4. "Enable JavaScript" message (+5)
        if analysis.has_enable_js_message {
            score += 5;
            reasons.push(RenderReason::EnableJavascriptMessage);
        }

        // 5. Body mostly scripts with very low content (+2)
        if analysis.script_count >= 5 && analysis.visible_text_length < 500 {
            score += 2;
            reasons.push(RenderReason::ScriptsDominated);
        }

        // 6. Content placeholders / skeletons (+2)
        if analysis.has_placeholders_only {
            score += 2;
            reasons.push(RenderReason::PlaceholdersOnly);
        }

        // 7. Hydration markers present but minimal text (+2)
        if !analysis.framework_markers.is_empty() && analysis.visible_text_length < 300 {
            score += 2;
            reasons.push(RenderReason::HydrationMarkerPresent);
        }

        // Negative weights (signals that HTTP scrape is already sufficient)
        // Useful main content (> 500 chars) -> -5
        if analysis.main_content_length >= 500 {
            score -= 5;
        }

        // Substantial visible text (> 3000 chars) -> -4
        if analysis.visible_text_length > 3000 {
            score -= 4;
        }

        // Multiple paragraphs (>= 4) -> -3
        if analysis.paragraph_count >= 4 {
            score -= 3;
        }

        let renderer = if score >= self.threshold {
            SelectedRenderer::Browser
        } else {
            reasons.clear();
            reasons.push(RenderReason::HttpContentSufficient);
            SelectedRenderer::Http
        };

        let diagnostics = RenderDiagnostics {
            visible_text_length: analysis.visible_text_length,
            script_count: analysis.script_count,
            paragraph_count: analysis.paragraph_count,
            link_count: analysis.link_count,
            framework_markers: analysis.framework_markers.clone(),
            render_score: score,
            render_threshold: self.threshold,
            selected_renderer: renderer,
            reasons: reasons.clone(),
        };

        RenderDecision {
            renderer,
            score,
            reasons,
            diagnostics,
        }
    }
}
