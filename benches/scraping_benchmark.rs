use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rustcrawl::extract::{clean_html, extract_main_content, html_to_markdown};
use rustcrawl::parser::{extract_links, ParsedDocument};
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

fn load_or_generate_pages() -> Vec<(&'static str, String)> {
    let pages = vec![
        ("50KB", generate_page(50 * 1024)),
        ("500KB", generate_page(500 * 1024)),
        ("2MB", generate_page(2 * 1024 * 1024)),
    ];
    pages
}

fn bench_html_parsing(c: &mut Criterion) {
    let pages = load_or_generate_pages();
    let mut group = c.benchmark_group("html_parsing");

    for (label, html) in &pages {
        let base_url = Url::parse("https://example.com/bench").unwrap();
        group.bench_with_input(BenchmarkId::from_parameter(label), html, |b, input| {
            b.iter(|| {
                let _doc = ParsedDocument::new(black_box(input), black_box(base_url.clone()));
            });
        });
    }
    group.finish();
}

fn bench_main_content_extraction(c: &mut Criterion) {
    let pages = load_or_generate_pages();
    let mut group = c.benchmark_group("main_content_extraction");

    for (label, html) in &pages {
        let base_url = Url::parse("https://example.com/bench").unwrap();
        let doc = ParsedDocument::new(html, base_url);
        group.bench_with_input(BenchmarkId::from_parameter(label), &doc, |b, input| {
            b.iter(|| {
                let mut warnings = Vec::new();
                let _main = extract_main_content(black_box(&input.html), black_box(&mut warnings));
            });
        });
    }
    group.finish();
}

fn bench_html_cleaning(c: &mut Criterion) {
    let pages = load_or_generate_pages();
    let mut group = c.benchmark_group("html_cleaning");

    for (label, html) in &pages {
        group.bench_with_input(BenchmarkId::from_parameter(label), html, |b, input| {
            b.iter(|| {
                let _cleaned = clean_html(black_box(input));
            });
        });
    }
    group.finish();
}

fn bench_markdown_conversion(c: &mut Criterion) {
    let pages = load_or_generate_pages();
    let mut group = c.benchmark_group("markdown_conversion");

    for (label, html) in &pages {
        let base_url = Url::parse("https://example.com/bench").unwrap();
        let cleaned = clean_html(html);
        group.bench_with_input(BenchmarkId::from_parameter(label), &cleaned, |b, input| {
            b.iter(|| {
                let _md = html_to_markdown(black_box(input), black_box(&base_url));
            });
        });
    }
    group.finish();
}

fn bench_link_extraction(c: &mut Criterion) {
    let pages = load_or_generate_pages();
    let mut group = c.benchmark_group("link_extraction");

    for (label, html) in &pages {
        let base_url = Url::parse("https://example.com/bench").unwrap();
        let doc = ParsedDocument::new(html, base_url);
        group.bench_with_input(BenchmarkId::from_parameter(label), &doc, |b, input| {
            b.iter(|| {
                let mut warnings = Vec::new();
                let _links = extract_links(black_box(input), black_box(&mut warnings));
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_html_parsing,
    bench_main_content_extraction,
    bench_html_cleaning,
    bench_markdown_conversion,
    bench_link_extraction
);
criterion_main!(benches);
