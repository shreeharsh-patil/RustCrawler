# RustCrawl

A production-grade, ultra-fast, and lightweight Firecrawl-style web scraper, crawler, and web-to-data engine built in pure Rust.

RustCrawl combines high-performance single-page HTTP scraping (Phase 1) with an asynchronous, multi-page website crawling engine (Phase 2) featuring bounded URL frontiers, per-host concurrency scheduling, lock-free deduplication, robots.txt compliance, sitemap discovery, SSRF protection, exponential backoff retries, and background crawl job management.

---

## Key Features

### Phase 1: High-Performance Single-Page Scraping
- **HTTP/1.1 & HTTP/2 Engine**: Asynchronous Tokio/Reqwest HTTP client with connection pooling, gzip, Brotli, and deflate decompression.
- **SSRF & Private Network Protection**: Comprehensive IP and hostname validation before connection; blocks IPv4/IPv6 loopback (`127.0.0.0/8`, `::1`), RFC 1918 private ranges (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `fc00::/7`), link-local (`169.254.0.0/16`, `fe80::/10`), CGNAT, documentation, multicast, IPv4-mapped IPv6 ranges, and cloud metadata endpoints (`169.254.169.254`, `metadata.google.internal`).
- **Safe Redirect Revalidation**: Follows up to 10 redirects while revalidating DNS and SSRF policies on every hop to prevent redirect-to-private-IP and DNS rebinding exploits.
- **Spec-Compliant HTML DOM Parsing**: Full DOM tree parsing using `scraper` and `html5ever`—no regex-based HTML extraction.
- **Lightweight Readability-Style Content Extraction**: Heuristic scoring engine prioritizing `<main>` and `<article>` tags while evaluating text density, paragraph density, heading counts, link density, and positive/negative semantic class/id terms.
- **High-Quality HTML-to-Markdown Converter**: Converts headings (h1-h6), paragraphs, formatting (bold, italic, strikethrough), tables with dividers, fenced code blocks with language tags, blockquotes, ordered/unordered/nested lists, horizontal rules, and links/images with resolved absolute URLs.
- **Deep Metadata & Structured Data Extraction**:
  - HTML `<title>`, `<meta name="description">`, `lang`, canonical URL, favicon, author, robots, published time, generator, theme-color.
  - Open Graph (`og:title`, `og:description`, `og:image`, `og:url`, `og:type`, `og:site_name`, `og:locale`).
  - Twitter / X cards (`twitter:card`, `twitter:title`, `twitter:description`, `twitter:image`, `twitter:site`, `twitter:creator`).
  - JSON-LD (`<script type="application/ld+json">`) parsing supporting objects, arrays, and `@graph` containers.

### Phase 2: Production-Grade Multi-Page Website Crawler
- **Bounded URL Frontier**: Memory-bounded async frontier with FIFO queueing, high-water mark tracking, and capacity protections (`MAX_FRONTIER_SIZE=10000`).
- **High-Throughput Lock-Free Deduplication**: Normalization of URL schemes, trailing slashes, default ports, path resolution (`..`, `.`), query parameter sorting, fragment removal, tracking parameter removal (`utm_*`, `fbclid`, `gclid`, `ref`), and static media/code asset extension filtering (`.png`, `.jpg`, `.css`, `.js`, `.pdf`, etc.).
- **Scheduler & Host Rate Limiting**: Global fetch concurrency limit (default 50), per-host semaphore concurrency limit (default 5), and per-host delay throttling without blocking other hosts.
- **robots.txt Compliance Engine**: Host-cached robots.txt parsing with User-Agent token matching, rule specificity (longest match wins), `Crawl-delay` parsing, and `Sitemap:` directive extraction.
- **Automated Sitemap Discovery**: Safe streaming XML parsing with `quick-xml` for `<urlset>` and recursive `<sitemapindex>` traversal with depth limits.
- **Resilient Retry Policy**: Exponential backoff with random jitter and `Retry-After` header support for transient errors and status codes (408, 425, 429, 500, 502, 503, 504).
- **Background Crawl Job Store**: In-memory job manager tracking background async execution, lock-free atomic `CrawlProgress` metrics, active cancellation tokens, job capacity limits, and automatic retention-based eviction.
- **Path & Domain Filtering**: Glob path inclusion/exclusion patterns (`*`, `**`, `?`) where exclusions take precedence, plus strict same-domain vs subdomain matching.

---

## Architecture

```text
Seed URL
    │
    ▼
[URL Syntax & SSRF Preflight]
    │
    ├──► [robots.txt & Sitemap Discovery Engine]
    │           │
    │           ▼ (Enqueue Sitemaps)
    ▼           ▼
[Bounded URL Frontier] ◄──────────────────────────────────────────────────┐
    │                                                                     │
    ▼ (Pop CrawlTarget)                                                   │
[Crawl Scheduler] (Global semaphore + Per-host semaphore + Per-host delay) │
    │                                                                     │
    ▼ (Process Target)                                                    │
[ScraperService Worker Pool]                                              │
    ├──► HTTP Fetch & SSRF Revalidation                                   │
    ├──► Exponential Backoff Retries (429, 500, 502, 503, 504)            │
    ├──► DOM Parsing & Markdown Conversion                                │
    ├──► Extract Links & Canonical URL                                    │
    │                                                                     │
    ▼                                                                     │
[Filter & Normalization Pipeline]                                         │
    ├──► Normalize URL & Strip Tracking Parameters                        │
    ├──► Static Asset Extension Filter (.jpg, .css, .js, .pdf, etc.)       │
    ├──► Domain / Subdomain Rules                                         │
    ├──► Include / Exclude Glob Path Patterns                             │
    ├──► Thread-Safe Deduplication (DashSet)                              │
    └──► Push Discovered URLs (Depth + 1) ────────────────────────────────┘
    │
    ▼
[Crawl Result Snapshot & In-Memory JobStore]
    │
    ├──► GET /v1/crawl/{job_id} (REST API)
    └──► CLI Terminal / JSON Output File
```

---

## Configuration

RustCrawl is configurable via environment variables or CLI flags:

| Environment Variable | Default Value | Description |
|---|---|---|
| `SERVER_HOST` | `0.0.0.0` | Host IP address to bind API daemon |
| `SERVER_PORT` | `3000` | Port for the HTTP API server |
| `REQUEST_TIMEOUT_SECONDS` | `15` | Target fetch timeout in seconds |
| `MAX_RESPONSE_SIZE_MB` | `10` | Maximum allowed HTML payload size (MB) |
| `MAX_REDIRECTS` | `10` | Maximum HTTP redirect hops allowed |
| `MAX_CONCURRENT_SCRAPES` | `100` | Global concurrency semaphore limit for single-page scrapes |
| `MAX_CONCURRENT_FETCHES` | `50` | Global concurrent fetch limit for multi-page crawling |
| `MAX_CONCURRENT_FETCHES_PER_HOST` | `5` | Per-host concurrent fetch limit |
| `DEFAULT_CRAWL_LIMIT` | `100` | Default maximum pages to crawl per job |
| `DEFAULT_MAX_DEPTH` | `3` | Default maximum crawl depth from seed (0 = seed only) |
| `MAX_FRONTIER_SIZE` | `10000` | Maximum queued targets in the frontier |
| `DEFAULT_MAX_RETRIES` | `2` | Number of retry attempts on transient network / 5xx errors |
| `DEFAULT_REQUEST_DELAY_MS` | `100` | Per-host polite crawl delay in milliseconds |
| `MAX_STORED_JOBS` | `100` | Maximum concurrent background crawl jobs retained in memory |
| `JOB_RETENTION_SECONDS` | `3600` | Time to retain completed/failed crawl jobs in seconds |
| `MAX_CRAWL_DURATION_SECONDS` | `300` | Maximum duration for a crawl job before timeout |
| `ALLOW_PRIVATE_NETWORKS` | `false` | Set to `true` to allow crawling internal/localhost addresses |
| `USER_AGENT` | `RustCrawler/0.1` | User-Agent header for HTTP requests |
| `RUST_LOG` | `info` | Log level (`trace`, `debug`, `info`, `warn`, `error`) |

---

## CLI Usage

RustCrawl provides a unified CLI with subcommands for serving the API, scraping single pages, and crawling entire websites.

### 1. Multi-Page Website Crawling
```bash
# Basic crawl with depth 2 and limit 50
rustcrawl crawl https://example.com --depth 2 --limit 50

# Crawl with path filtering, saving JSON result to file
rustcrawl crawl https://example.com \
  --limit 200 \
  --depth 3 \
  --include "/docs/**" \
  --exclude "/docs/private/**" \
  --allow-subdomains \
  --output crawl_result.json

# Fast intranet crawl with custom delay and formats
rustcrawl crawl https://example.com \
  --format markdown \
  --format metadata \
  --format links \
  --delay 50 \
  --output result.json
```

### 2. Single-Page Scraping
```bash
# Scrape markdown content to stdout
rustcrawl scrape https://example.com

# Scrape specific formats and output formatted JSON
rustcrawl scrape https://example.com -f markdown -f metadata -f links --json
```

### 3. Running the REST API Server
```bash
rustcrawl serve --host 0.0.0.0 --port 3000
```

---

## REST API Reference

### 1. `POST /v1/crawl`
Submits a background crawl job starting from a seed URL.

**Request Body:**
```json
{
  "url": "https://example.com",
  "limit": 100,
  "max_depth": 3,
  "max_duration_seconds": 300,
  "allow_subdomains": false,
  "include_paths": ["/docs/**"],
  "exclude_paths": ["/docs/admin/**"],
  "respect_robots_txt": true,
  "use_sitemap": true,
  "formats": ["markdown", "metadata", "links"],
  "only_main_content": true,
  "request_delay_ms": 100
}
```

**Response (`202 Accepted`):**
```json
{
  "success": true,
  "job_id": "9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d",
  "status": "queued"
}
```

---

### 2. `GET /v1/crawl/{job_id}`
Polls the status and pages collected by a crawl job.

**Response (`200 OK`):**
```json
{
  "success": true,
  "data": {
    "job_id": "9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d",
    "status": "completed",
    "seed_url": "https://example.com",
    "stats": {
      "pages_discovered": 45,
      "pages_crawled": 42,
      "pages_succeeded": 42,
      "pages_failed": 0,
      "pages_skipped": 3,
      "pages_blocked_by_robots": 0,
      "deduplication_hits": 128,
      "retries_count": 0,
      "bytes_downloaded": 482910,
      "max_depth_reached": 2,
      "elapsed_ms": 1250,
      "started_at": "2026-08-25T18:00:00Z",
      "finished_at": "2026-08-25T18:00:01Z"
    },
    "pages": [
      {
        "url": "https://example.com/docs/intro",
        "final_url": "https://example.com/docs/intro",
        "depth": 1,
        "status_code": 200,
        "markdown": "# Introduction\n\nWelcome to the documentation...",
        "timings": {
          "fetch_time_ms": 25,
          "parse_time_ms": 2,
          "total_time_ms": 27
        }
      }
    ]
  }
}
```

---

### 3. `DELETE /v1/crawl/{job_id}`
Cancels an active crawl job immediately.

**Response (`200 OK`):**
```json
{
  "success": true,
  "job_id": "9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d",
  "status": "cancelled"
}
```

---

### 4. `POST /v1/scrape`
Synchronously fetches and parses a single web page.

**Request Body:**
```json
{
  "url": "https://example.com",
  "formats": ["markdown", "html", "clean_html", "links", "images", "metadata"],
  "only_main_content": true
}
```

**Response (`200 OK`):**
```json
{
  "success": true,
  "data": {
    "url": "https://example.com",
    "final_url": "https://example.com",
    "markdown": "# Example Domain\n\nThis domain is for use in illustrative examples in documents.",
    "metadata": {
      "title": "Example Domain",
      "description": null,
      "language": "en"
    },
    "links": [
      {
        "text": "More information...",
        "url": "https://www.iana.org/domains/example",
        "is_external": true
      }
    ],
    "images": [],
    "http": {
      "status": 200,
      "content_type": "text/html; charset=UTF-8",
      "content_length": 1256
    },
    "timings": {
      "fetch_time_ms": 42,
      "parse_time_ms": 3,
      "clean_time_ms": 1,
      "markdown_time_ms": 2,
      "total_time_ms": 48
    },
    "warnings": []
  }
}
```

---

### 5. `GET /health`
Returns health status and service metrics.

**Response (`200 OK`):**
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 120
}
```

---

## Performance Benchmarks

Audited on modern multi-core hardware:

| Benchmark Operation | Throughput | Latency / Notes |
|---|---|---|
| **URL Deduplication & Normalization** | **146,302 ops/sec** | 50,000 URLs deduplicated in 341.7 ms |
| **Frontier Enqueue** | **4,063,818 ops/sec** | 50,000 items enqueued in 12.3 ms |
| **Frontier Dequeue** | **9,948,467 ops/sec** | 50,000 items dequeued in 5.0 ms |
| **Glob Path Filtering** | **3,255,908 checks/sec** | 50,000 paths checked in 15.3 ms |
| **robots.txt Compliance Checks** | **6,233,948 checks/sec** | 50,000 paths evaluated in 8.0 ms |
| **Streaming XML Sitemap Parsing** | **307,040 urls/sec** | 10,000 URLs extracted in 32.5 ms |
| **Local 100-Page Crawl** | **994.5 pages/sec** | 101 pages in 101.5 ms |
| **Local 500-Page Crawl** | **2,340.6 pages/sec** | 501 pages in 214.0 ms |
| **Local 1,000-Page Crawl** | **3,138.8 pages/sec** | 1,001 pages in 318.9 ms |

---

## Testing & Quality Assurance

```bash
# Run all unit and integration tests (67 tests)
cargo test --lib

# Verify zero linter warnings
cargo clippy --all-targets --all-features -- -D warnings

# Check code formatting
cargo fmt --check

# Run benchmark audit
cargo run --bin measure
```

---

## License

MIT License. Designed & built with Google Antigravity.
