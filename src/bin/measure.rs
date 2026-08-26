use axum::routing::get;
use axum::Router;
use rustcrawl::crawl::dedupe::{normalize_crawl_url, UrlDeduplicator};
use rustcrawl::crawl::frontier::CrawlFrontier;
use rustcrawl::crawl::models::{CrawlOptions, CrawlTarget, DiscoverySource};
use rustcrawl::crawl::robots::ParsedRobots;
use rustcrawl::crawl::sitemap::parse_sitemap_xml;
use rustcrawl::crawl::CrawlerService;
use rustcrawl::extract::{clean_html, extract_main_content, html_to_markdown};
use rustcrawl::parser::ParsedDocument;
use rustcrawl::service::ScraperService;
use rustcrawl::utils::patterns::is_path_allowed;
use rustcrawl::Config;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use url::Url;

fn generate_page(target_bytes: usize) -> String {
    let header = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <title>Benchmark Document - Rust Web Scraping Engine</title>
</head>
<body>
    <header>
        <nav><a href="/">Home</a> | <a href="/docs">Docs</a></nav>
    </header>
    <main>
        <article class="post-content">
            <h1>High-Throughput Web Scraping</h1>
"#;

    let footer = r#"
        </article>
    </main>
    <footer><p>&copy; 2026 RustCrawl</p></footer>
</body>
</html>"#;

    let mut body = String::with_capacity(target_bytes);
    body.push_str(header);

    let mut idx = 1;
    while body.len() + footer.len() + 512 < target_bytes {
        body.push_str(&format!(
            r#"
            <section id="sec-{idx}">
                <h2>Section {idx}: Benchmark Title</h2>
                <p>Paragraph demonstrating realistic content with <a href="/link-{idx}">internal link</a> and <a href="https://ext.org/{idx}">external link</a>.</p>
                <table>
                    <thead><tr><th>Col A</th><th>Col B</th></tr></thead>
                    <tbody><tr><td>Val A</td><td>Val B</td></tr></tbody>
                </table>
            </section>
"#
        ));
        idx += 1;
    }
    body.push_str(footer);
    body
}

async fn spawn_benchmark_server(page_count: usize) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let mut app = Router::new();

    // Generate root page linking to all pages
    let mut root_html =
        String::from("<html><head><title>Root</title></head><body><h1>Benchmark Site</h1>");
    for i in 1..=page_count {
        root_html.push_str(&format!(r#"<a href="/page/{i}">Page {i}</a><br/>"#));
    }
    root_html.push_str("</body></html>");

    let root_html_arc = Arc::new(root_html);
    let root_clone = root_html_arc.clone();
    app = app.route(
        "/",
        get(move || {
            let r = root_clone.clone();
            async move { (*r).clone() }
        }),
    );

    // Generate individual pages linking to siblings
    for i in 1..=page_count {
        let next_page = if i < page_count { i + 1 } else { 1 };
        let page_html = format!(
            r#"<html><head><title>Page {i}</title></head><body><h1>Page {i}</h1><p>Content for benchmark page {i}.</p><a href="/page/{next_page}">Next</a></body></html>"#
        );
        app = app.route(&format!("/page/{i}"), get(move || async move { page_html }));
    }

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (addr, handle)
}

#[tokio::main]
async fn main() {
    println!("============================================================");
    println!("              RUSTCRAWL BENCHMARK & PERFORMANCE AUDIT       ");
    println!("============================================================");

    // ==========================================
    // 1. Single Page Parsing (Phase 1 Benchmarks)
    // ==========================================
    let sizes = [
        ("50 KB", 50 * 1024),
        ("500 KB", 500 * 1024),
        ("2 MB", 2 * 1024 * 1024),
    ];
    let base_url = Url::parse("https://example.com/bench").unwrap();

    println!("\n--- [Phase 1] DOM Parsing & Transformation Benchmarks ---");
    for (label, size) in sizes {
        let html = generate_page(size);
        let iterations = if size > 1024 * 1024 { 20 } else { 100 };

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = ParsedDocument::new(&html, base_url.clone());
        }
        let parse_avg = start.elapsed().as_secs_f64() * 1000.0 / iterations as f64;

        let doc = ParsedDocument::new(&html, base_url.clone());
        let start = Instant::now();
        for _ in 0..iterations {
            let mut warnings = Vec::new();
            let _ = extract_main_content(&doc.html, &mut warnings);
        }
        let extract_avg = start.elapsed().as_secs_f64() * 1000.0 / iterations as f64;

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = clean_html(&html);
        }
        let clean_avg = start.elapsed().as_secs_f64() * 1000.0 / iterations as f64;

        let cleaned = clean_html(&html);
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = html_to_markdown(&cleaned, &base_url);
        }
        let md_avg = start.elapsed().as_secs_f64() * 1000.0 / iterations as f64;

        println!(
            "{label:<8} | DOM: {:>6.2} ms | Main: {:>6.2} ms | Clean: {:>6.2} ms | MD: {:>6.2} ms",
            parse_avg, extract_avg, clean_avg, md_avg
        );
    }

    // ==========================================
    // 2. URL Deduplication Benchmarks (Phase 2)
    // ==========================================
    println!("\n--- [Phase 2] URL Deduplication Benchmarks ---");
    for count in [1_000, 10_000, 50_000] {
        let dedupe = UrlDeduplicator::new();
        let urls: Vec<String> = (0..count)
            .map(|i| {
                format!("https://example.com/docs/section/{i}?utm_source=twitter&ref=blog#{i}")
            })
            .collect();

        let start = Instant::now();
        for u_str in &urls {
            let u = Url::parse(u_str).unwrap();
            let norm = normalize_crawl_url(&u, false, true);
            dedupe.insert(&norm);
        }
        let elapsed = start.elapsed();
        let ops_per_sec = (count as f64) / elapsed.as_secs_f64();
        println!(
            "Dedupe {count:>6} URLs: {:>7.2} ms ({:>10.0} ops/sec)",
            elapsed.as_secs_f64() * 1000.0,
            ops_per_sec
        );
    }

    // ==========================================
    // 3. Frontier Enqueue / Dequeue Benchmarks
    // ==========================================
    println!("\n--- [Phase 2] Frontier Enqueue / Dequeue Benchmarks ---");
    let frontier = CrawlFrontier::new(100_000);
    let count = 50_000;
    let targets: Vec<CrawlTarget> = (0..count)
        .map(|i| CrawlTarget {
            url: Url::parse(&format!("https://example.com/item/{i}")).unwrap(),
            normalized_url: format!("https://example.com/item/{i}"),
            depth: (i % 4) as u32,
            parent_url: None,
            discovered_from: DiscoverySource::HtmlLink,
        })
        .collect();

    let start = Instant::now();
    for target in targets {
        let _ = frontier.try_push(target);
    }
    let enqueue_elapsed = start.elapsed();

    let start = Instant::now();
    let mut dequeued = 0;
    while frontier.try_pop().is_some() {
        dequeued += 1;
    }
    let dequeue_elapsed = start.elapsed();

    println!(
        "Frontier {count} Enqueues: {:>7.2} ms ({:>10.0} ops/sec)",
        enqueue_elapsed.as_secs_f64() * 1000.0,
        (count as f64) / enqueue_elapsed.as_secs_f64()
    );
    println!(
        "Frontier {dequeued} Dequeues: {:>7.2} ms ({:>10.0} ops/sec)",
        dequeue_elapsed.as_secs_f64() * 1000.0,
        (dequeued as f64) / dequeue_elapsed.as_secs_f64()
    );

    // ==========================================
    // 4. Domain & Path Filtering Benchmarks
    // ==========================================
    println!("\n--- [Phase 2] Domain & Path Filtering Benchmarks ---");
    let includes = vec!["/docs/**".to_string(), "/blog/**".to_string()];
    let excludes = vec!["/admin/**".to_string(), "/docs/private/**".to_string()];
    let paths: Vec<String> = (0..50_000)
        .map(|i| {
            if i % 3 == 0 {
                format!("/docs/section/{i}/page")
            } else if i % 3 == 1 {
                format!("/admin/users/{i}")
            } else {
                format!("/other/path/{i}")
            }
        })
        .collect();

    let start = Instant::now();
    let mut allowed_count = 0;
    for path in &paths {
        if is_path_allowed(path, &includes, &excludes) {
            allowed_count += 1;
        }
    }
    let filter_elapsed = start.elapsed();
    println!(
        "Path Filtering 50,000 paths: {:>7.2} ms ({:>10.0} checks/sec, allowed: {allowed_count})",
        filter_elapsed.as_secs_f64() * 1000.0,
        50_000.0 / filter_elapsed.as_secs_f64()
    );

    // ==========================================
    // 5. robots.txt & Sitemap Parsing
    // ==========================================
    println!("\n--- [Phase 2] robots.txt & Sitemap Parsing ---");
    let robots_txt = r#"
User-agent: *
Disallow: /admin/
Disallow: /private/
Disallow: /api/keys/
Allow: /api/keys/public
Crawl-delay: 1.5
Sitemap: https://example.com/sitemap.xml
"#;
    let parsed_robots = ParsedRobots::parse(robots_txt);
    let start = Instant::now();
    for i in 0..50_000 {
        let path = if i % 2 == 0 {
            "/docs/page"
        } else {
            "/admin/keys"
        };
        let _ = parsed_robots.is_path_allowed(path, "RustCrawler/0.1");
    }
    let robots_elapsed = start.elapsed();
    println!(
        "robots.txt 50,000 checks: {:>7.2} ms ({:>10.0} checks/sec)",
        robots_elapsed.as_secs_f64() * 1000.0,
        50_000.0 / robots_elapsed.as_secs_f64()
    );

    // Sitemap parsing with 10,000 URLs
    let mut sitemap_xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#,
    );
    for i in 0..10_000 {
        sitemap_xml.push_str(&format!(
            "<url><loc>https://example.com/page/{i}</loc></url>"
        ));
    }
    sitemap_xml.push_str("</urlset>");

    let start = Instant::now();
    let entries = parse_sitemap_xml(&sitemap_xml);
    let sitemap_elapsed = start.elapsed();
    println!(
        "Sitemap parsing 10,000 URLs: {:>7.2} ms ({:>10.0} urls/sec, parsed: {})",
        sitemap_elapsed.as_secs_f64() * 1000.0,
        10_000.0 / sitemap_elapsed.as_secs_f64(),
        entries.len()
    );

    // ==========================================
    // 6. End-to-End Local Crawl Throughput
    // ==========================================
    println!("\n--- [Phase 2] End-to-End Local Crawl Throughput ---");
    for page_target in [100, 500, 1000] {
        let (addr, server_handle) = spawn_benchmark_server(page_target).await;

        let config = Config {
            request_timeout_seconds: 10,
            max_concurrent_fetches: 50,
            max_concurrent_fetches_per_host: 50,
            allow_private_networks: true,
            ..Default::default()
        };
        let scraper = Arc::new(ScraperService::new(config).unwrap());
        let crawler = CrawlerService::new(scraper);

        let seed_url = format!("http://{addr}/");
        let options = CrawlOptions {
            limit: page_target,
            max_depth: 3,
            respect_robots_txt: false,
            use_sitemap: false,
            request_delay_ms: 0,
            ..Default::default()
        };

        let cancel = CancellationToken::new();
        let start = Instant::now();
        let result = crawler.crawl(&seed_url, options, cancel).await.unwrap();
        let elapsed = start.elapsed();
        let pages_per_sec = (result.stats.pages_crawled as f64) / elapsed.as_secs_f64();

        println!(
            "Crawl {:>4} Pages: {:>7.2} ms | Pages Crawled: {:>4} | Succeeded: {:>4} | {:>7.1} pages/sec | Bytes: {:>7}",
            page_target,
            elapsed.as_secs_f64() * 1000.0,
            result.stats.pages_crawled,
            result.stats.pages_succeeded,
            pages_per_sec,
            result.stats.bytes_downloaded
        );

        server_handle.abort();
    }

    println!("\n============================================================");
    println!("                   BENCHMARK RUN COMPLETED                   ");
    println!("============================================================");
}
