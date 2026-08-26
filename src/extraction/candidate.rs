use crate::extraction::models::{ExtractedField, ExtractionSource};
use crate::models::ScrapeResult;
use scraper::{Html, Selector};
use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub struct CandidatePool {
    pub candidates: Vec<(String, ExtractedField<Value>)>,
}

impl CandidatePool {
    pub fn new() -> Self {
        Self {
            candidates: Vec::new(),
        }
    }

    pub fn add(
        &mut self,
        key: impl Into<String>,
        value: Value,
        confidence: f32,
        source: ExtractionSource,
        excerpt: Option<String>,
    ) {
        self.candidates.push((
            key.into(),
            ExtractedField {
                value,
                confidence,
                source,
                excerpt,
            },
        ));
    }

    /// Collects candidate key-value pairs from all available signals in a ScrapeResult.
    pub fn from_scrape_result(result: &ScrapeResult) -> Self {
        let mut pool = Self::new();

        // 1. JSON-LD structured data
        if let Some(ref meta) = result.metadata {
            for json_ld in &meta.json_ld {
                extract_json_candidates(&mut pool, json_ld, ExtractionSource::JsonLd, "");
            }
        }

        // 2. Direct JSON data (e.g. from JSON document parser or API)
        if let Some(ref json_val) = result.json {
            extract_json_candidates(&mut pool, json_val, ExtractionSource::EmbeddedJson, "");
        }

        // 3. Network JSON responses (from Phase 3 CDP network capture)
        if let Some(ref net_resps) = result.network_responses {
            for net in net_resps {
                if let Some(ref json_val) = net.body {
                    extract_json_candidates(&mut pool, json_val, ExtractionSource::NetworkJson, "");
                }
            }
        }

        // 4. OpenGraph & Twitter Metadata
        if let Some(ref meta) = result.metadata {
            if let Some(ref og) = meta.open_graph {
                if let Some(ref t) = og.title {
                    pool.add(
                        "title",
                        Value::String(t.clone()),
                        0.85,
                        ExtractionSource::MetaTag,
                        Some(t.clone()),
                    );
                    pool.add(
                        "name",
                        Value::String(t.clone()),
                        0.80,
                        ExtractionSource::MetaTag,
                        Some(t.clone()),
                    );
                }
                if let Some(ref d) = og.description {
                    pool.add(
                        "description",
                        Value::String(d.clone()),
                        0.85,
                        ExtractionSource::MetaTag,
                        Some(d.clone()),
                    );
                    pool.add(
                        "summary",
                        Value::String(d.clone()),
                        0.80,
                        ExtractionSource::MetaTag,
                        Some(d.clone()),
                    );
                }
                if let Some(ref img) = og.image {
                    pool.add(
                        "image",
                        Value::String(img.clone()),
                        0.85,
                        ExtractionSource::MetaTag,
                        Some(img.clone()),
                    );
                    pool.add(
                        "image_url",
                        Value::String(img.clone()),
                        0.85,
                        ExtractionSource::MetaTag,
                        Some(img.clone()),
                    );
                }
                if let Some(ref u) = og.url {
                    pool.add(
                        "url",
                        Value::String(u.clone()),
                        0.90,
                        ExtractionSource::MetaTag,
                        Some(u.clone()),
                    );
                }
                if let Some(ref s) = og.site_name {
                    pool.add(
                        "site_name",
                        Value::String(s.clone()),
                        0.80,
                        ExtractionSource::MetaTag,
                        Some(s.clone()),
                    );
                    pool.add(
                        "company",
                        Value::String(s.clone()),
                        0.70,
                        ExtractionSource::MetaTag,
                        Some(s.clone()),
                    );
                }
            }

            if let Some(ref tw) = meta.twitter {
                if let Some(ref t) = tw.title {
                    pool.add(
                        "title",
                        Value::String(t.clone()),
                        0.80,
                        ExtractionSource::MetaTag,
                        Some(t.clone()),
                    );
                }
                if let Some(ref d) = tw.description {
                    pool.add(
                        "description",
                        Value::String(d.clone()),
                        0.80,
                        ExtractionSource::MetaTag,
                        Some(d.clone()),
                    );
                }
                if let Some(ref img) = tw.image {
                    pool.add(
                        "image",
                        Value::String(img.clone()),
                        0.80,
                        ExtractionSource::MetaTag,
                        Some(img.clone()),
                    );
                }
            }

            if let Some(ref t) = meta.title {
                pool.add(
                    "title",
                    Value::String(t.clone()),
                    0.85,
                    ExtractionSource::MetaTag,
                    Some(t.clone()),
                );
                pool.add(
                    "name",
                    Value::String(t.clone()),
                    0.80,
                    ExtractionSource::MetaTag,
                    Some(t.clone()),
                );
            }
            if let Some(ref d) = meta.description {
                pool.add(
                    "description",
                    Value::String(d.clone()),
                    0.85,
                    ExtractionSource::MetaTag,
                    Some(d.clone()),
                );
            }
            if let Some(ref a) = meta.author {
                pool.add(
                    "author",
                    Value::String(a.clone()),
                    0.85,
                    ExtractionSource::MetaTag,
                    Some(a.clone()),
                );
                pool.add(
                    "creator",
                    Value::String(a.clone()),
                    0.80,
                    ExtractionSource::MetaTag,
                    Some(a.clone()),
                );
            }
            if let Some(ref p) = meta.published_time {
                pool.add(
                    "published_time",
                    Value::String(p.clone()),
                    0.85,
                    ExtractionSource::MetaTag,
                    Some(p.clone()),
                );
                pool.add(
                    "date_published",
                    Value::String(p.clone()),
                    0.85,
                    ExtractionSource::MetaTag,
                    Some(p.clone()),
                );
            }
        }

        // 5. Tables (HTML tables and CSV/document tables)
        if let Some(ref tables) = result.tables {
            for table in tables {
                for row in &table.rows {
                    for (i, cell) in row.iter().enumerate() {
                        if let Some(header) = table.headers.get(i) {
                            if !header.trim().is_empty() && !cell.trim().is_empty() {
                                let val_parsed = parse_raw_value(cell);
                                pool.add(
                                    header.trim().to_string(),
                                    val_parsed,
                                    0.80,
                                    ExtractionSource::HtmlTable,
                                    Some(format!("{header}: {cell}")),
                                );
                            }
                        }
                    }
                }
            }
        }

        // 6. DOM Label / Value pairs and Definition Lists (if HTML available)
        if let Some(ref html_content) = result.html {
            extract_dom_key_values(&mut pool, html_content);
        }

        pool
    }
}

fn extract_json_candidates(
    pool: &mut CandidatePool,
    val: &Value,
    source: ExtractionSource,
    prefix: &str,
) {
    match val {
        Value::Object(map) => {
            for (k, v) in map {
                let full_key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };

                let excerpt = serde_json::to_string(v).ok().map(|s| {
                    if s.len() > 120 {
                        format!("{}...", &s[..120])
                    } else {
                        s
                    }
                });

                pool.add(k.clone(), v.clone(), 0.95, source, excerpt.clone());
                if full_key != *k {
                    pool.add(full_key.clone(), v.clone(), 0.90, source, excerpt);
                }

                // Recursively extract nested objects
                if v.is_object() {
                    extract_json_candidates(pool, v, source, &full_key);
                }
            }
        }
        Value::Array(arr) => {
            for (idx, item) in arr.iter().enumerate() {
                if item.is_object() {
                    let array_prefix = if prefix.is_empty() {
                        format!("[{idx}]")
                    } else {
                        format!("{prefix}[{idx}]")
                    };
                    extract_json_candidates(pool, item, source, &array_prefix);
                }
            }
        }
        _ => {}
    }
}

fn extract_dom_key_values(pool: &mut CandidatePool, html_content: &str) {
    let document = Html::parse_document(html_content);

    // Definition lists: <dl><dt>Label</dt><dd>Value</dd></dl>
    if let Ok(dl_sel) = Selector::parse("dl") {
        if let (Ok(dt_sel), Ok(dd_sel)) = (Selector::parse("dt"), Selector::parse("dd")) {
            for dl in document.select(&dl_sel) {
                let dts: Vec<_> = dl
                    .select(&dt_sel)
                    .map(|el| el.text().collect::<String>().trim().to_string())
                    .collect();
                let dds: Vec<_> = dl
                    .select(&dd_sel)
                    .map(|el| el.text().collect::<String>().trim().to_string())
                    .collect();

                for (dt, dd) in dts.iter().zip(dds.iter()) {
                    if !dt.is_empty() && !dd.is_empty() {
                        pool.add(
                            dt.clone(),
                            parse_raw_value(dd),
                            0.75,
                            ExtractionSource::HtmlText,
                            Some(format!("{dt}: {dd}")),
                        );
                    }
                }
            }
        }
    }
}

pub fn parse_raw_value(raw: &str) -> Value {
    let trimmed = raw.trim();
    if let Ok(num) = trimmed.parse::<i64>() {
        Value::Number(num.into())
    } else if let Ok(flt) = trimmed.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(flt) {
            Value::Number(n)
        } else {
            Value::String(trimmed.to_string())
        }
    } else if trimmed.eq_ignore_ascii_case("true") {
        Value::Bool(true)
    } else if trimmed.eq_ignore_ascii_case("false") {
        Value::Bool(false)
    } else if trimmed.eq_ignore_ascii_case("null") {
        Value::Null
    } else {
        Value::String(trimmed.to_string())
    }
}
