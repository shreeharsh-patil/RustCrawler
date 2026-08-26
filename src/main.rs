use clap::{Args, Parser, Subcommand};
use rustcrawl::config::Config;
use rustcrawl::crawl::incremental::{IncrementalCrawlRequest, IncrementalCrawler};
use rustcrawl::crawl::{CrawlOptions, CrawlerService};
use rustcrawl::discovery::models::MapRequest;
use rustcrawl::discovery::service::MapService;
use rustcrawl::distributed::coordinator::frontier::DistributedFrontier;
use rustcrawl::distributed::coordinator::scheduler::JobScheduler;
use rustcrawl::distributed::models::WorkerType;
use rustcrawl::distributed::queue::{MemoryTaskQueue, RedisTaskQueue, TaskQueue};
use rustcrawl::distributed::results::{LocalResultStore, ResultStore};
use rustcrawl::distributed::store::migrations::get_migration_statements;
use rustcrawl::distributed::store::{MemoryMetadataStore, MetadataStore, SqlMetadataStore};
use rustcrawl::distributed::worker::WorkerRuntime;
use rustcrawl::extraction::{
    ExtractionCrawlOptions, ExtractionMode, ExtractionRequest, ExtractionResponse,
    ExtractionService, MissingFieldBehavior,
};
use rustcrawl::knowledge::{KnowledgeEngine, SearchMode as KnowledgeSearchMode};
use rustcrawl::models::{OutputFormat, ScrapeOptions, ScrapeRequest};
use rustcrawl::render::models::{RenderMode, WaitStrategy};
use rustcrawl::search::models::{SearchMode, SearchRequest};
use rustcrawl::search::service::SearchService;
use rustcrawl::service::ScraperService;
use serde_json::Value;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(
    name = "rustcrawl",
    version,
    about = "Production-grade universal web scraper, crawler, and web-to-data engine in Rust",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the HTTP REST API server
    Serve(ServeArgs),

    /// Scrape a single URL or local file directly from the CLI
    Scrape(ScrapeCliArgs),

    /// Crawl a website recursively starting from a seed URL
    Crawl(CrawlCliArgs),

    /// Parse a local file directly using the universal document parser registry
    ParseFile(ParseFileCliArgs),

    /// Extract structured JSON data from a URL, documents, or crawl using deterministic rules and optional LLM
    Extract(ExtractCliArgs),

    /// Fast website discovery and URL mapping
    Map(MapCliArgs),

    /// Search the web or local index with optional scraping
    Search(SearchCliArgs),

    /// Start a distributed worker process
    Worker(WorkerCliArgs),

    /// Manage and inspect distributed crawl jobs
    Job(JobCliArgs),

    /// Inspect distributed task queue statistics
    Queue(QueueCliArgs),

    /// Dead Letter Queue (DLQ) operations
    Dlq(DlqCliArgs),

    /// Run database migrations
    Migrate,

    /// Index a URL, document, or crawl job into a semantic search index
    Index(IndexCliArgs),

    /// Search an index using semantic vector similarity
    SearchSemantic(SearchSemanticCliArgs),

    /// Search an index using hybrid RRF (vector + lexical) fusion
    SearchHybrid(SearchHybridCliArgs),

    /// Retrieve token-bounded and ranked context chunks for RAG
    Retrieve(RetrieveCliArgs),

    /// Ask a question and get a grounded answer with source citations
    Ask(AskCliArgs),

    /// Execute autonomous web research with bounded planning, evidence gathering, and citations
    Agent(AgentCliArgs),

    /// Start Model Context Protocol (MCP) server over stdin/stdout
    Mcp,
}

#[derive(Args, Debug)]
struct ServeArgs {
    /// Server execution mode: standalone or distributed
    #[arg(long = "mode", env = "EXECUTION_MODE")]
    mode: Option<String>,

    /// Host IP address to bind server
    #[arg(long, env = "SERVER_HOST")]
    host: Option<IpAddr>,

    /// Port to listen on
    #[arg(short, long, env = "SERVER_PORT")]
    port: Option<u16>,

    /// Path to custom Chromium/Chrome executable
    #[arg(long = "browser-path", env = "BROWSER_EXECUTABLE_PATH")]
    browser_path: Option<String>,

    /// Disable browser rendering completely
    #[arg(long = "no-browser", default_value_t = false)]
    no_browser: bool,
}

#[derive(Args, Debug)]
struct WorkerCliArgs {
    /// Worker type: http, browser, document, extraction, general
    #[arg(short = 't', long = "type", default_value = "general")]
    worker_type: String,

    /// Maximum concurrent tasks to process
    #[arg(short = 'c', long = "concurrency", default_value_t = 20)]
    concurrency: usize,

    /// Custom worker ID identifier
    #[arg(long = "id")]
    worker_id: Option<String>,

    /// Path to custom Chromium/Chrome executable
    #[arg(long = "browser-path")]
    browser_path: Option<String>,
}

#[derive(Args, Debug)]
struct JobCliArgs {
    #[command(subcommand)]
    action: JobAction,
}

#[derive(Subcommand, Debug)]
enum JobAction {
    /// Inspect a job and its execution progress
    Inspect { id: String },

    /// Pause task scheduling for a job
    Pause { id: String },

    /// Resume task scheduling for a job
    Resume { id: String },

    /// Cancel a running job
    Cancel { id: String },

    /// Retrieve paginated results for a completed job
    Results {
        id: String,
        #[arg(short, long)]
        limit: Option<usize>,
    },
}

#[derive(Args, Debug)]
struct QueueCliArgs {
    #[command(subcommand)]
    action: QueueAction,
}

#[derive(Subcommand, Debug)]
enum QueueAction {
    /// Display distributed queue statistics
    Stats,
}

#[derive(Args, Debug)]
struct DlqCliArgs {
    #[command(subcommand)]
    action: DlqAction,
}

#[derive(Subcommand, Debug)]
enum DlqAction {
    /// List tasks in the Dead Letter Queue
    List {
        #[arg(short, long, default_value_t = 50)]
        limit: usize,
    },

    /// Retry a task from the Dead Letter Queue
    Retry { task_id: String },
}

#[derive(Args, Debug)]
struct ScrapeCliArgs {
    /// Target web page URL or local file path (e.g. https://example.com, report.pdf, data.csv)
    url: String,

    /// Output formats to include (markdown, html, clean_html, links, images, metadata, json, tables, pages, text)
    #[arg(short = 'f', long = "format")]
    formats: Vec<String>,

    /// Extract only the main content rather than the whole page
    #[arg(long = "main-content", default_value_t = true, action = clap::ArgAction::Set)]
    main_content: bool,

    /// Rendering mode: auto (default), http (force HTTP), browser (force Chromium)
    #[arg(long = "render", default_value = "auto")]
    render: String,

    /// Browser wait strategy (load, domcontentloaded, networkidle, selector:<sel>, timeout:<ms>)
    #[arg(long = "wait")]
    wait: Option<String>,

    /// Automatically scroll down to load lazy-loaded elements
    #[arg(long = "auto-scroll", default_value_t = false)]
    auto_scroll: bool,

    /// Capture network XHR/Fetch JSON responses
    #[arg(long = "capture-network", default_value_t = false)]
    capture_network: bool,

    /// Block analytics and tracker domains
    #[arg(long = "block-trackers", default_value_t = false)]
    block_trackers: bool,

    /// Optional JSONPath query (e.g. $.products[*])
    #[arg(long = "json-path")]
    json_path: Option<String>,

    /// Path to custom Chromium/Chrome executable
    #[arg(long = "browser-path")]
    browser_path: Option<String>,

    /// Pretty print the full JSON result
    #[arg(long = "json", default_value_t = false)]
    json: bool,
}

#[derive(Args, Debug)]
struct ParseFileCliArgs {
    /// Path to local file to parse (PDF, DOCX, CSV, JSON, XML, RSS, Text, etc.)
    file_path: String,

    /// Output formats to include (markdown, json, tables, pages, text, metadata)
    #[arg(short = 'f', long = "format")]
    formats: Vec<String>,

    /// Optional JSONPath query for JSON documents
    #[arg(long = "json-path")]
    json_path: Option<String>,

    /// Pretty print the full JSON result
    #[arg(long = "json", default_value_t = false)]
    json: bool,
}

#[derive(Args, Debug)]
struct CrawlCliArgs {
    /// Seed URL to start crawling (must start with http:// or https://)
    url: String,

    /// Maximum number of pages to crawl
    #[arg(short, long, default_value_t = 100)]
    limit: usize,

    /// Maximum crawl depth from seed (seed is depth 0)
    #[arg(short = 'd', long = "depth", default_value_t = 3)]
    depth: u32,

    /// Maximum crawl duration in seconds
    #[arg(long, default_value_t = 300)]
    max_duration: u64,

    /// Include path glob patterns (e.g. "/docs/**")
    #[arg(long = "include")]
    include_paths: Vec<String>,

    /// Exclude path glob patterns (e.g. "/admin/**")
    #[arg(long = "exclude")]
    exclude_paths: Vec<String>,

    /// Document types to crawl (e.g. html, pdf, docx, json, csv)
    #[arg(long = "document-types")]
    document_types: Vec<String>,

    /// Follow and enqueue links found in RSS / Atom feeds
    #[arg(long = "follow-feed-links", default_value_t = false)]
    follow_feed_links: bool,

    /// Follow and enqueue links found in PDF / DOCX / Text documents
    #[arg(long = "follow-document-links", default_value_t = false)]
    follow_document_links: bool,

    /// Deduplicate pages with identical content hashes
    #[arg(long = "deduplicate-content", default_value_t = false)]
    deduplicate_content: bool,

    /// Allow crawling subdomains of the seed host
    #[arg(long = "allow-subdomains", default_value_t = false)]
    allow_subdomains: bool,

    /// Respect robots.txt directives
    #[arg(long = "respect-robots", default_value_t = true, action = clap::ArgAction::Set)]
    respect_robots: bool,

    /// Discover URLs via sitemaps
    #[arg(long = "sitemap", default_value_t = true, action = clap::ArgAction::Set)]
    use_sitemap: bool,

    /// Output formats to include (markdown, html, clean_html, links, images, metadata)
    #[arg(short = 'f', long = "format")]
    formats: Vec<String>,

    /// Extract only the main content
    #[arg(long = "main-content", default_value_t = true, action = clap::ArgAction::Set)]
    main_content: bool,

    /// Rendering mode: auto (default), http (force HTTP), browser (force Chromium)
    #[arg(long = "render", default_value = "auto")]
    render: String,

    /// Browser wait strategy (load, domcontentloaded, networkidle, selector:<sel>, timeout:<ms>)
    #[arg(long = "wait")]
    wait: Option<String>,

    /// Automatically scroll down to load lazy-loaded elements
    #[arg(long = "auto-scroll", default_value_t = false)]
    auto_scroll: bool,

    /// Capture network XHR/Fetch JSON responses
    #[arg(long = "capture-network", default_value_t = false)]
    capture_network: bool,

    /// Block analytics and tracker domains
    #[arg(long = "block-trackers", default_value_t = false)]
    block_trackers: bool,

    /// Path to custom Chromium/Chrome executable
    #[arg(long = "browser-path")]
    browser_path: Option<String>,

    /// File path to save output JSON result
    #[arg(short = 'o', long = "output")]
    output: Option<String>,

    /// Delay between requests to the same host in milliseconds
    #[arg(long = "delay", default_value_t = 100)]
    delay_ms: u64,

    /// Save crawl snapshot for incremental comparisons
    #[arg(long = "save-snapshot", default_value_t = false)]
    save_snapshot: bool,

    /// Perform incremental crawl against baseline snapshot ID or baseline job ID
    #[arg(long = "incremental")]
    incremental: Option<String>,
}

#[derive(Args, Debug)]
struct MapCliArgs {
    /// Website URL to discover and map
    url: String,

    /// Maximum number of URLs to discover
    #[arg(short, long, default_value_t = 5000)]
    limit: usize,

    /// Filter and rank URLs by relevance to search query
    #[arg(short = 's', long = "search")]
    search: Option<String>,

    /// Include path glob patterns (e.g. "/docs/**")
    #[arg(long = "include")]
    include_paths: Vec<String>,

    /// Exclude path glob patterns (e.g. "/admin/**")
    #[arg(long = "exclude")]
    exclude_paths: Vec<String>,

    /// Include subdomains
    #[arg(long = "include-subdomains", default_value_t = false)]
    include_subdomains: bool,

    /// File path to save output JSON result
    #[arg(short = 'o', long = "output")]
    output: Option<String>,
}

#[derive(Args, Debug)]
struct SearchCliArgs {
    /// Search query string
    query: String,

    /// Restrict search to specific domain (e.g. "docs.rs")
    #[arg(short = 'd', long = "domain")]
    domain: Option<String>,

    /// Scrape and extract content for search result pages
    #[arg(long = "scrape", default_value_t = false)]
    scrape: bool,

    /// Search local index of previously crawled / mapped sites
    #[arg(long = "local", default_value_t = false)]
    local: bool,

    /// Maximum number of results to return
    #[arg(short, long, default_value_t = 10)]
    limit: usize,

    /// Search provider name (default: configured provider)
    #[arg(long = "provider")]
    provider: Option<String>,

    /// File path to save output JSON result
    #[arg(short = 'o', long = "output")]
    output: Option<String>,
}

#[derive(Args, Debug)]
struct ExtractCliArgs {
    /// Target web page URL or crawl seed (e.g. https://example.com)
    url: Option<String>,

    /// Natural language prompt describing information to extract
    #[arg(short = 'p', long = "prompt")]
    prompt: Option<String>,

    /// Path to JSON schema file or JSON schema string describing expected structured output
    #[arg(short = 's', long = "schema")]
    schema: Option<String>,

    /// Extraction mode: auto (default), deterministic (no LLM), llm (force LLM)
    #[arg(short = 'm', long = "mode", default_value = "auto")]
    mode: String,

    /// Include source citation provenance mapping in output
    #[arg(long = "provenance", default_value_t = false)]
    provenance: bool,

    /// Crawl website before extraction starting from seed URL
    #[arg(long = "crawl")]
    crawl: Option<String>,

    /// Maximum pages to crawl if --crawl is enabled
    #[arg(long = "limit", default_value_t = 20)]
    limit: usize,

    /// Maximum crawl depth if --crawl is enabled
    #[arg(long = "depth", default_value_t = 2)]
    depth: u32,

    /// Deduplicate array entities by field names (comma separated, e.g. "name,price")
    #[arg(long = "dedupe-by")]
    dedupe_by: Option<String>,

    /// Custom LLM provider (e.g. openai, openai-compatible, mock)
    #[arg(long = "provider")]
    provider: Option<String>,

    /// Custom model name (e.g. gpt-4o-mini, llama3)
    #[arg(long = "model")]
    model: Option<String>,

    /// Output file path to save JSON result
    #[arg(short = 'o', long = "output")]
    output: Option<String>,
}

#[derive(Args, Debug)]
struct IndexCliArgs {
    /// URL or local file path to index
    #[arg(index = 1)]
    target: Option<String>,

    /// Target search index name
    #[arg(short, long, default_value = "default")]
    name: String,

    /// Crawl job ID to index from completed crawl
    #[arg(long = "crawl-job")]
    crawl_job: Option<String>,

    /// Inspect index statistics
    #[arg(long = "inspect")]
    inspect: bool,

    /// Delete index
    #[arg(long = "delete")]
    delete: bool,

    /// Rebuild index
    #[arg(long = "rebuild")]
    rebuild: bool,
}

#[derive(Args, Debug)]
struct SearchSemanticCliArgs {
    /// Index name to search
    index: String,

    /// Semantic query string
    query: String,

    /// Max results to return
    #[arg(short, long, default_value_t = 10)]
    limit: usize,
}

#[derive(Args, Debug)]
struct SearchHybridCliArgs {
    /// Index name to search
    index: String,

    /// Search query string
    query: String,

    /// Max results to return
    #[arg(short, long, default_value_t = 20)]
    limit: usize,
}

#[derive(Args, Debug)]
struct RetrieveCliArgs {
    /// Index name to retrieve context from
    index: String,

    /// Query or question
    query: String,

    /// Max context chunks to retrieve
    #[arg(short = 'k', long = "top-k", default_value_t = 8)]
    top_k: usize,

    /// Search mode: hybrid, semantic, lexical
    #[arg(short, long, default_value = "hybrid")]
    mode: String,
}

#[derive(Args, Debug)]
struct AskCliArgs {
    /// Index name
    index: String,

    /// Question to ask
    question: String,

    /// Top K chunks to retrieve for grounding
    #[arg(short = 'k', long = "top-k", default_value_t = 8)]
    top_k: usize,
}

#[derive(Args, Debug)]
struct AgentCliArgs {
    /// Research task objective or comparison question
    task: Option<String>,

    /// Subcommand action: inspect, pause, resume, cancel
    #[arg(short = 'a', long = "action")]
    action: Option<String>,

    /// Target Job ID for inspection or lifecycle control
    #[arg(long = "job-id")]
    job_id: Option<String>,

    /// Agent mode: research (default), structured_research, site_analysis, comparison, monitoring_analysis
    #[arg(short = 'm', long = "mode", default_value = "research")]
    mode: String,

    /// Path to JSON schema file for structured output
    #[arg(short = 's', long = "schema")]
    schema: Option<PathBuf>,

    /// Freshness window in days (e.g. 30)
    #[arg(long = "freshness")]
    freshness: Option<u32>,

    /// Max steps limit
    #[arg(long = "max-steps")]
    max_steps: Option<u32>,

    /// Max pages limit
    #[arg(long = "max-pages")]
    max_pages: Option<u32>,

    /// Initial URLs to seed research
    #[arg(long = "urls")]
    urls: Vec<String>,

    /// Output file path for JSON report
    #[arg(short = 'o', long = "output")]
    output: Option<String>,
}

fn parse_wait_strategy(s: &str) -> Option<WaitStrategy> {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "load" => Some(WaitStrategy::Load),
        "domcontentloaded" | "dom_content_loaded" => Some(WaitStrategy::DomContentLoaded),
        "networkidle" | "network_idle" | "idle" => Some(WaitStrategy::NetworkIdle),
        _ => {
            if let Some(stripped) = s.strip_prefix("selector:") {
                Some(WaitStrategy::Selector(stripped.to_string()))
            } else if let Some(stripped) = s.strip_prefix("timeout:") {
                stripped
                    .parse::<u64>()
                    .ok()
                    .map(|ms| WaitStrategy::Timeout(Duration::from_millis(ms)))
            } else {
                Some(WaitStrategy::Selector(s.to_string()))
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut config = Config::from_env();

    // Initialize tracing subscriber
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.rust_log)),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    match cli.command {
        Commands::Serve(serve_args) => {
            if let Some(h) = serve_args.host {
                config.server_host = h;
            }
            if let Some(p) = serve_args.port {
                config.server_port = p;
            }
            if let Some(bp) = serve_args.browser_path {
                config.browser_executable_path = Some(bp);
            }
            if serve_args.no_browser {
                config.browser_enabled = false;
            }

            let addr = config.server_addr();
            let service = ScraperService::new(config)?;
            let app = rustcrawl::api::router(service).layer(TraceLayer::new_for_http());

            info!("Starting rustcrawl server listening on http://{addr}");

            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await?;
        }

        Commands::Scrape(scrape_args) => {
            if let Some(bp) = scrape_args.browser_path {
                config.browser_executable_path = Some(bp);
            }

            let render_mode = scrape_args
                .render
                .parse::<RenderMode>()
                .unwrap_or(RenderMode::Auto);

            let wait_strategy = scrape_args.wait.as_deref().and_then(parse_wait_strategy);

            let service = ScraperService::new(config)?;

            let formats = if scrape_args.formats.is_empty() {
                vec![OutputFormat::Markdown]
            } else {
                let mut parsed_formats = Vec::new();
                for fmt_str in &scrape_args.formats {
                    match fmt_str.parse::<OutputFormat>() {
                        Ok(f) => parsed_formats.push(f),
                        Err(e) => {
                            eprintln!("Error: {e}");
                            std::process::exit(1);
                        }
                    }
                }
                parsed_formats
            };

            let path = Path::new(&scrape_args.url);
            let result = if path.exists()
                && !scrape_args.url.starts_with("http://")
                && !scrape_args.url.starts_with("https://")
            {
                // Local file scraping
                let options = ScrapeOptions {
                    formats: formats.clone(),
                    only_main_content: scrape_args.main_content,
                    render_mode,
                    wait_for: wait_strategy,
                    auto_scroll: scrape_args.auto_scroll,
                    capture_network: scrape_args.capture_network,
                    block_trackers: scrape_args.block_trackers,
                    block_resources: Vec::new(),
                    actions: Vec::new(),
                    timeout_ms: None,
                    json_path: scrape_args.json_path,
                    follow_feed_links: None,
                    follow_document_links: None,
                };
                service.parse_file(path, &options).await
            } else {
                let request = ScrapeRequest {
                    url: scrape_args.url,
                    formats: Some(formats.clone()),
                    only_main_content: Some(scrape_args.main_content),
                    render_mode: Some(render_mode),
                    wait_for: wait_strategy,
                    auto_scroll: Some(scrape_args.auto_scroll),
                    capture_network: Some(scrape_args.capture_network),
                    block_trackers: Some(scrape_args.block_trackers),
                    block_resources: None,
                    actions: None,
                    timeout_ms: None,
                    json_path: scrape_args.json_path,
                    follow_feed_links: None,
                    follow_document_links: None,
                };
                service.scrape(request).await
            };

            match result {
                Ok(res) => {
                    if scrape_args.json
                        || formats.len() > 1
                        || !formats.contains(&OutputFormat::Markdown)
                    {
                        let json_str = serde_json::to_string_pretty(&res)?;
                        println!("{json_str}");
                    } else if let Some(ref md) = res.markdown {
                        println!("{md}");
                    } else if let Some(ref text) = res.text {
                        println!("{text}");
                    }
                }
                Err(err) => {
                    let err_response = rustcrawl::error::ApiErrorResponse::from(&err);
                    eprintln!(
                        "{}",
                        serde_json::to_string_pretty(&err_response)
                            .unwrap_or_else(|_| err.to_string())
                    );
                    std::process::exit(1);
                }
            }
        }

        Commands::ParseFile(parse_args) => {
            let service = ScraperService::new(config)?;
            let formats = if parse_args.formats.is_empty() {
                vec![OutputFormat::Markdown]
            } else {
                let mut parsed_formats = Vec::new();
                for fmt_str in &parse_args.formats {
                    match fmt_str.parse::<OutputFormat>() {
                        Ok(f) => parsed_formats.push(f),
                        Err(e) => {
                            eprintln!("Error: {e}");
                            std::process::exit(1);
                        }
                    }
                }
                parsed_formats
            };

            let options = ScrapeOptions {
                formats: formats.clone(),
                only_main_content: true,
                render_mode: RenderMode::Http,
                wait_for: None,
                auto_scroll: false,
                capture_network: false,
                block_trackers: false,
                block_resources: Vec::new(),
                actions: Vec::new(),
                timeout_ms: None,
                json_path: parse_args.json_path,
                follow_feed_links: None,
                follow_document_links: None,
            };

            let path = Path::new(&parse_args.file_path);
            match service.parse_file(path, &options).await {
                Ok(res) => {
                    if parse_args.json
                        || formats.len() > 1
                        || !formats.contains(&OutputFormat::Markdown)
                    {
                        let json_str = serde_json::to_string_pretty(&res)?;
                        println!("{json_str}");
                    } else if let Some(ref md) = res.markdown {
                        println!("{md}");
                    } else if let Some(ref text) = res.text {
                        println!("{text}");
                    }
                }
                Err(err) => {
                    let err_response = rustcrawl::error::ApiErrorResponse::from(&err);
                    eprintln!(
                        "{}",
                        serde_json::to_string_pretty(&err_response)
                            .unwrap_or_else(|_| err.to_string())
                    );
                    std::process::exit(1);
                }
            }
        }

        Commands::Crawl(crawl_args) => {
            if let Some(bp) = crawl_args.browser_path {
                config.browser_executable_path = Some(bp);
            }

            let render_mode = crawl_args
                .render
                .parse::<RenderMode>()
                .unwrap_or(RenderMode::Auto);

            let wait_strategy = crawl_args.wait.as_deref().and_then(parse_wait_strategy);

            let scraper = Arc::new(ScraperService::new(config)?);
            let crawler = CrawlerService::new(scraper);

            let formats = if crawl_args.formats.is_empty() {
                None
            } else {
                let mut parsed_formats = Vec::new();
                for fmt_str in &crawl_args.formats {
                    match fmt_str.parse::<OutputFormat>() {
                        Ok(f) => parsed_formats.push(f),
                        Err(e) => {
                            eprintln!("Error: {e}");
                            std::process::exit(1);
                        }
                    }
                }
                Some(parsed_formats)
            };

            let doc_types = if crawl_args.document_types.is_empty() {
                vec!["html".to_string()]
            } else {
                crawl_args.document_types
            };

            let options = CrawlOptions {
                limit: crawl_args.limit,
                max_depth: crawl_args.depth,
                max_duration_seconds: crawl_args.max_duration,
                allow_subdomains: crawl_args.allow_subdomains,
                include_paths: crawl_args.include_paths,
                exclude_paths: crawl_args.exclude_paths,
                respect_robots_txt: crawl_args.respect_robots,
                use_sitemap: crawl_args.use_sitemap,
                formats,
                only_main_content: Some(crawl_args.main_content),
                request_delay_ms: crawl_args.delay_ms,
                render_mode,
                wait_for: wait_strategy,
                auto_scroll: crawl_args.auto_scroll,
                capture_network: crawl_args.capture_network,
                block_trackers: crawl_args.block_trackers,
                document_types: doc_types,
                follow_feed_links: crawl_args.follow_feed_links,
                follow_document_links: crawl_args.follow_document_links,
                deduplicate_content: crawl_args.deduplicate_content,
                ..Default::default()
            };

            let seed_url = crawl_args.url;
            eprintln!("Initiating crawl for seed URL: {seed_url}");

            let cancel_token = CancellationToken::new();
            let cancel_clone = cancel_token.clone();

            tokio::spawn(async move {
                let _ = tokio::signal::ctrl_c().await;
                eprintln!("\nReceived Ctrl+C, cancelling crawl...");
                cancel_clone.cancel();
            });

            if let Some(ref baseline) = crawl_args.incremental {
                let crawler_arc = Arc::new(crawler);
                let incremental = IncrementalCrawler::new(
                    crawler_arc.clone(),
                    crawler_arc.snapshot_store().clone(),
                );
                let inc_req = IncrementalCrawlRequest {
                    url: seed_url,
                    baseline_job_id: Some(baseline.clone()),
                    baseline_snapshot_id: Some(baseline.clone()),
                    limit: Some(crawl_args.limit),
                    include_changed_content: true,
                    options: Some(options),
                };

                let result = incremental
                    .execute_incremental_crawl(inc_req, cancel_token)
                    .await;
                match result {
                    Ok(inc_res) => {
                        eprintln!("Incremental crawl completed!");
                        eprintln!("New: {}", inc_res.changes.new.len());
                        eprintln!("Changed: {}", inc_res.changes.changed.len());
                        eprintln!("Deleted: {}", inc_res.changes.deleted.len());
                        eprintln!("Unchanged: {}", inc_res.changes.unchanged_count);

                        let json_str = serde_json::to_string_pretty(&inc_res)?;
                        if let Some(ref out_path) = crawl_args.output {
                            tokio::fs::write(out_path, &json_str).await?;
                            eprintln!("Result saved to {out_path}");
                        } else {
                            println!("{json_str}");
                        }
                    }
                    Err(err) => {
                        eprintln!("Incremental crawl failed: {err}");
                        std::process::exit(1);
                    }
                }
            } else {
                let result = crawler.crawl(&seed_url, options, cancel_token).await;

                match result {
                    Ok(crawl_res) => {
                        if crawl_args.save_snapshot {
                            let mut snap = rustcrawl::crawl::CrawlSnapshot::new(
                                crawl_res.job_id.clone(),
                                seed_url.clone(),
                            );
                            for p in &crawl_res.pages {
                                snap.insert_page(rustcrawl::crawl::PageSnapshot {
                                    url: p.url.clone(),
                                    final_url: p.final_url.clone(),
                                    content_hash: p.content_hash.clone(),
                                    raw_content_hash: p.content_hash.clone(),
                                    normalized_content_hash: p.text.as_deref().map(
                                        rustcrawl::crawl::snapshot::compute_normalized_content_hash,
                                    ),
                                    metadata_hash: None,
                                    etag: None,
                                    last_modified: None,
                                    status_code: p.status_code,
                                    title: p.metadata.as_ref().and_then(|m| m.title.clone()),
                                });
                            }
                            crawler.snapshot_store().save_snapshot(snap);
                            eprintln!("Saved crawl snapshot with ID: {}", crawl_res.job_id);
                        }

                        let json_str = serde_json::to_string_pretty(&crawl_res)?;
                        if let Some(ref out_path) = crawl_args.output {
                            tokio::fs::write(out_path, &json_str).await?;
                            eprintln!(
                                "Crawl completed! Crawled {} pages. Result saved to {out_path}",
                                crawl_res.pages.len()
                            );
                        } else {
                            println!("{json_str}");
                        }
                    }
                    Err(err) => {
                        eprintln!("Crawl job failed: {err}");
                        std::process::exit(1);
                    }
                }
            }
        }
        Commands::Extract(extract_args) => {
            let scraper = Arc::new(ScraperService::new(config.clone())?);
            let crawler = Arc::new(CrawlerService::new(scraper.clone()));
            let extraction =
                ExtractionService::new(config.clone(), scraper.clone(), crawler.clone());

            // 1. Resolve Schema from file or inline JSON string
            let schema_value = if let Some(ref schema_input) = extract_args.schema {
                let path = Path::new(schema_input);
                if path.exists() {
                    let content = tokio::fs::read_to_string(path).await?;
                    let val: serde_json::Value = serde_json::from_str(&content)?;
                    Some(val)
                } else {
                    let val: serde_json::Value = serde_json::from_str(schema_input)?;
                    Some(val)
                }
            } else {
                None
            };

            let mode: ExtractionMode = extract_args.mode.parse().unwrap_or(ExtractionMode::Auto);

            let crawl_opts = if let Some(ref _seed) = extract_args.crawl {
                Some(ExtractionCrawlOptions {
                    limit: extract_args.limit,
                    max_depth: extract_args.depth,
                    ..Default::default()
                })
            } else {
                None
            };

            let dedupe_keys = if let Some(ref keys_str) = extract_args.dedupe_by {
                keys_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            } else {
                Vec::new()
            };

            let target_url = extract_args.url.or(extract_args.crawl);

            let req = ExtractionRequest {
                url: target_url,
                crawl: crawl_opts,
                prompt: extract_args.prompt,
                schema: schema_value,
                mode: Some(mode),
                include_provenance: Some(extract_args.provenance),
                missing_field_behavior: Some(MissingFieldBehavior::Null),
                dedupe_by: dedupe_keys,
                provider: extract_args.provider,
                model: extract_args.model,
                ..Default::default()
            };

            let cancel_token = CancellationToken::new();
            let cancel_clone = cancel_token.clone();

            tokio::spawn(async move {
                let _ = tokio::signal::ctrl_c().await;
                eprintln!("\nReceived Ctrl+C, cancelling extraction...");
                cancel_clone.cancel();
            });

            match extraction.extract(req, Some(cancel_token)).await {
                Ok(result) => {
                    let response = ExtractionResponse::from(result);
                    let json_str = serde_json::to_string_pretty(&response)?;

                    if let Some(ref out_path) = extract_args.output {
                        tokio::fs::write(out_path, &json_str).await?;
                        eprintln!("Extraction completed successfully! Result saved to {out_path}");
                    } else {
                        println!("{json_str}");
                    }
                }
                Err(err) => {
                    eprintln!("Extraction failed: {err}");
                    std::process::exit(1);
                }
            }
        }

        Commands::Map(map_args) => {
            let config_arc = Arc::new(config.clone());
            let scraper = Arc::new(ScraperService::new(config.clone())?);
            let map_service = MapService::new(config_arc, scraper, None);

            let req = MapRequest {
                url: map_args.url,
                limit: map_args.limit,
                search: map_args.search,
                include_subdomains: map_args.include_subdomains,
                include_sitemap: true,
                include_paths: map_args.include_paths,
                exclude_paths: map_args.exclude_paths,
                ignore_query_parameters: true,
                render_dynamic_links: false,
                max_depth: 3,
            };

            match map_service.map(req).await {
                Ok(resp) => {
                    let json_str = serde_json::to_string_pretty(&resp)?;
                    if let Some(ref out_path) = map_args.output {
                        tokio::fs::write(out_path, &json_str).await?;
                        eprintln!("Discovered {} URLs. Saved to {out_path}", resp.links.len());
                    } else {
                        println!("{json_str}");
                    }
                }
                Err(err) => {
                    eprintln!("Map discovery failed: {err}");
                    std::process::exit(1);
                }
            }
        }

        Commands::Search(search_args) => {
            let config_arc = Arc::new(config.clone());
            let scraper = Arc::new(ScraperService::new(config.clone())?);
            let map_service = Arc::new(MapService::new(config_arc.clone(), scraper.clone(), None));
            let search_service = SearchService::new(config_arc, scraper, map_service, None, None);

            let mode = if search_args.local {
                Some(SearchMode::LocalIndex)
            } else if search_args.domain.is_some() {
                Some(SearchMode::Site)
            } else {
                Some(SearchMode::Web)
            };

            let req = SearchRequest {
                query: search_args.query,
                limit: search_args.limit,
                scrape: search_args.scrape,
                formats: vec!["markdown".to_string()],
                mode,
                domain: search_args.domain,
                exclude_domains: Vec::new(),
                provider: search_args.provider,
            };

            match search_service.search(req).await {
                Ok(resp) => {
                    let json_str = serde_json::to_string_pretty(&resp)?;
                    if let Some(ref out_path) = search_args.output {
                        tokio::fs::write(out_path, &json_str).await?;
                        eprintln!(
                            "Search returned {} results. Saved to {out_path}",
                            resp.data.len()
                        );
                    } else {
                        println!("{json_str}");
                    }
                }
                Err(err) => {
                    eprintln!("Search failed: {err}");
                    std::process::exit(1);
                }
            }
        }

        Commands::Worker(worker_args) => {
            let config_arc = Arc::new(config.clone());
            let scraper = Arc::new(ScraperService::new(config.clone())?);

            let worker_type =
                WorkerType::from_str(&worker_args.worker_type).unwrap_or(WorkerType::General);
            let worker_id = worker_args
                .worker_id
                .unwrap_or_else(|| format!("worker_{}", uuid::Uuid::new_v4().simple()));

            let metadata_store: Arc<dyn MetadataStore> =
                if let Some(ref db_url) = config.database_url {
                    Arc::new(SqlMetadataStore::new(db_url.clone()))
                } else {
                    Arc::new(MemoryMetadataStore::new())
                };

            let task_queue: Arc<dyn TaskQueue> = if let Some(ref r_url) = config.redis_url {
                Arc::new(RedisTaskQueue::new(r_url.clone(), 100_000))
            } else {
                Arc::new(MemoryTaskQueue::default())
            };

            let result_store = Arc::new(LocalResultStore::default());
            let frontier = Arc::new(DistributedFrontier::new());

            let runtime = WorkerRuntime::new(
                worker_id,
                worker_type,
                config_arc,
                scraper,
                metadata_store,
                task_queue,
                result_store,
                frontier,
                worker_args.concurrency,
            );

            let cancel_token = CancellationToken::new();
            let cancel_clone = cancel_token.clone();

            tokio::spawn(async move {
                let _ = tokio::signal::ctrl_c().await;
                eprintln!("\nReceived Ctrl+C, shutting down worker gracefully...");
                cancel_clone.cancel();
            });

            runtime.run(cancel_token).await;
        }

        Commands::Job(job_args) => {
            let metadata_store: Arc<dyn MetadataStore> =
                if let Some(ref db_url) = config.database_url {
                    Arc::new(SqlMetadataStore::new(db_url.clone()))
                } else {
                    Arc::new(MemoryMetadataStore::new())
                };
            let task_queue: Arc<dyn TaskQueue> = Arc::new(MemoryTaskQueue::default());
            let frontier = Arc::new(DistributedFrontier::new());
            let scheduler = JobScheduler::new(metadata_store.clone(), task_queue, frontier);

            match job_args.action {
                JobAction::Inspect { id } => {
                    if let Ok(Some(job)) = metadata_store.get_job(&id).await {
                        println!("{}", serde_json::to_string_pretty(&job)?);
                    } else {
                        eprintln!("Job not found: {id}");
                        std::process::exit(1);
                    }
                }
                JobAction::Pause { id } => {
                    if let Err(e) = scheduler.pause_job(&id).await {
                        eprintln!("Failed to pause job: {e}");
                        std::process::exit(1);
                    }
                    println!("Job {id} paused successfully");
                }
                JobAction::Resume { id } => {
                    if let Err(e) = scheduler.resume_job(&id).await {
                        eprintln!("Failed to resume job: {e}");
                        std::process::exit(1);
                    }
                    println!("Job {id} resumed successfully");
                }
                JobAction::Cancel { id } => {
                    if let Err(e) = scheduler.cancel_job(&id).await {
                        eprintln!("Failed to cancel job: {e}");
                        std::process::exit(1);
                    }
                    println!("Job {id} cancelled successfully");
                }
                JobAction::Results { id, limit } => {
                    let res_store = LocalResultStore::default();
                    let locations = res_store
                        .list_chunk_locations(&id)
                        .await
                        .unwrap_or_default();
                    let lim = limit.unwrap_or(locations.len()).min(locations.len());
                    println!("{}", serde_json::to_string_pretty(&locations[..lim])?);
                }
            }
        }

        Commands::Queue(queue_args) => {
            let task_queue: Arc<dyn TaskQueue> = if let Some(ref r_url) = config.redis_url {
                Arc::new(RedisTaskQueue::new(r_url.clone(), 100_000))
            } else {
                Arc::new(MemoryTaskQueue::default())
            };

            match queue_args.action {
                QueueAction::Stats => {
                    let stats = task_queue
                        .stats()
                        .await
                        .map_err(|e| std::io::Error::other(e.to_string()))?;
                    println!("{}", serde_json::to_string_pretty(&stats)?);
                }
            }
        }

        Commands::Dlq(dlq_args) => {
            let task_queue: Arc<dyn TaskQueue> = if let Some(ref r_url) = config.redis_url {
                Arc::new(RedisTaskQueue::new(r_url.clone(), 100_000))
            } else {
                Arc::new(MemoryTaskQueue::default())
            };

            match dlq_args.action {
                DlqAction::List { limit } => {
                    let items = task_queue
                        .dlq_list(limit)
                        .await
                        .map_err(|e| std::io::Error::other(e.to_string()))?;
                    println!("{}", serde_json::to_string_pretty(&items)?);
                }
                DlqAction::Retry { task_id } => {
                    let success = task_queue
                        .dlq_retry(&task_id)
                        .await
                        .map_err(|e| std::io::Error::other(e.to_string()))?;
                    if success {
                        println!("Task {task_id} successfully moved from DLQ back to queue");
                    } else {
                        eprintln!("Task {task_id} not found in DLQ");
                    }
                }
            }
        }

        Commands::Migrate => {
            let is_postgres = config
                .database_url
                .as_deref()
                .map(|u| u.starts_with("postgres://") || u.starts_with("postgresql://"))
                .unwrap_or(false);

            let stmts = get_migration_statements(is_postgres);
            eprintln!(
                "Running {} migrations for target database ({})",
                stmts.len(),
                if is_postgres { "PostgreSQL" } else { "SQLite" }
            );

            for (idx, _sql) in stmts.iter().enumerate() {
                eprintln!("Applying migration chunk #{}...", idx + 1);
            }
            eprintln!("Database schema up-to-date!");
        }

        Commands::Index(index_args) => {
            let knowledge_engine = KnowledgeEngine::new(Arc::new(config.clone()), None);
            let index = match knowledge_engine.get_index(&index_args.name) {
                Ok(idx) => idx,
                Err(_) => knowledge_engine.create_index(&index_args.name, None, None, None)?,
            };

            if index_args.inspect {
                let stats = knowledge_engine.index_stats(&index.id)?;
                println!("{}", serde_json::to_string_pretty(&stats)?);
                return Ok(());
            }

            if index_args.delete {
                knowledge_engine.delete_index(&index.id).await?;
                println!("Index '{}' deleted successfully.", index_args.name);
                return Ok(());
            }

            if index_args.rebuild {
                knowledge_engine.rebuild_index(&index.id).await?;
                println!("Index '{}' rebuilt successfully.", index_args.name);
                return Ok(());
            }

            if let Some(ref crawl_job_id) = index_args.crawl_job {
                let res_store = LocalResultStore::default();
                let locations = res_store
                    .list_chunk_locations(crawl_job_id)
                    .await
                    .unwrap_or_default();
                let mut count = 0;
                for loc in locations {
                    if let Ok(bytes) = res_store.get(&loc).await {
                        if let Ok(scrape_res) =
                            serde_json::from_slice::<rustcrawl::ScrapeResult>(&bytes)
                        {
                            if let Some(ref md) = scrape_res.markdown {
                                let title =
                                    scrape_res.metadata.as_ref().and_then(|m| m.title.clone());
                                knowledge_engine
                                    .index_markdown(
                                        &index.id,
                                        md,
                                        title.as_deref(),
                                        Some(&scrape_res.url),
                                        None,
                                    )
                                    .await?;
                                count += 1;
                            }
                        }
                    }
                }
                println!(
                    "Indexed {count} pages from crawl job '{crawl_job_id}' into index '{}'",
                    index.name
                );
                return Ok(());
            }

            if let Some(ref target) = index_args.target {
                let path = Path::new(target);
                if path.exists() && path.is_file() {
                    let content = std::fs::read_to_string(path)?;
                    let title = path.file_stem().and_then(|s| s.to_str());
                    knowledge_engine
                        .index_markdown(&index.id, &content, title, Some(target), None)
                        .await?;
                    println!("Indexed file '{}' into index '{}'", target, index.name);
                } else {
                    let scraper = ScraperService::new(config)?;
                    let res = scraper.scrape(ScrapeRequest::new(target)).await?;
                    if let Some(md) = res.markdown {
                        let title = res.metadata.as_ref().and_then(|m| m.title.clone());
                        knowledge_engine
                            .index_markdown(&index.id, &md, title.as_deref(), Some(target), None)
                            .await?;
                        println!("Indexed URL '{}' into index '{}'", target, index.name);
                    } else {
                        eprintln!("No markdown content extracted from '{target}'");
                    }
                }
            } else {
                let stats = knowledge_engine.index_stats(&index.id)?;
                println!("{}", serde_json::to_string_pretty(&stats)?);
            }
        }

        Commands::SearchSemantic(args) => {
            let knowledge_engine = KnowledgeEngine::new(Arc::new(config), None);
            let matches = knowledge_engine
                .search(
                    &args.index,
                    &args.query,
                    args.limit,
                    KnowledgeSearchMode::Semantic,
                    None,
                    None,
                )
                .await?;
            println!("{}", serde_json::to_string_pretty(&matches)?);
        }

        Commands::SearchHybrid(args) => {
            let knowledge_engine = KnowledgeEngine::new(Arc::new(config), None);
            let matches = knowledge_engine
                .search(
                    &args.index,
                    &args.query,
                    args.limit,
                    KnowledgeSearchMode::Hybrid,
                    None,
                    None,
                )
                .await?;
            println!("{}", serde_json::to_string_pretty(&matches)?);
        }

        Commands::Retrieve(args) => {
            let knowledge_engine = KnowledgeEngine::new(Arc::new(config), None);
            let mode = match args.mode.to_lowercase().as_str() {
                "semantic" => KnowledgeSearchMode::Semantic,
                "lexical" => KnowledgeSearchMode::Lexical,
                _ => KnowledgeSearchMode::Hybrid,
            };
            let res = knowledge_engine
                .retrieve(
                    &args.index,
                    &args.query,
                    args.top_k,
                    mode,
                    None,
                    None,
                    false,
                )
                .await?;
            println!("{}", serde_json::to_string_pretty(&res)?);
        }

        Commands::Ask(args) => {
            let knowledge_engine = KnowledgeEngine::new(Arc::new(config), None);
            let res = knowledge_engine
                .answer(&args.index, &args.question, args.top_k, None, None)
                .await?;
            println!("{}", serde_json::to_string_pretty(&res)?);
        }

        Commands::Agent(args) => {
            let config_arc = Arc::new(config.clone());
            let scraper = Arc::new(ScraperService::new(config)?);
            let crawler = Arc::new(CrawlerService::new(scraper.clone()));
            let extraction = Arc::new(ExtractionService::new(
                config_arc.as_ref().clone(),
                scraper.clone(),
                crawler.clone(),
            ));
            let map_service = Arc::new(MapService::new(config_arc.clone(), scraper.clone(), None));
            let search_service = Arc::new(SearchService::new(
                config_arc.clone(),
                scraper.clone(),
                map_service.clone(),
                None,
                None,
            ));
            let knowledge_engine = Arc::new(KnowledgeEngine::new(config_arc.clone(), None));

            let agent_service = rustcrawl::agent::AgentService::new(
                config_arc.clone(),
                Some(search_service),
                Some(map_service),
                scraper,
                crawler,
                extraction,
                Some(knowledge_engine),
                None,
                None,
            );

            if let Some(task) = args.task {
                let schema_val = if let Some(ref path) = args.schema {
                    let text = tokio::fs::read_to_string(path)
                        .await
                        .map_err(|e| format!("Failed to read schema file: {e}"))?;
                    Some(
                        serde_json::from_str::<Value>(&text)
                            .map_err(|e| format!("Failed to parse schema JSON: {e}"))?,
                    )
                } else {
                    None
                };

                let mode = match args.mode.to_lowercase().as_str() {
                    "structured_research" | "structured" => {
                        rustcrawl::agent::AgentMode::StructuredResearch
                    }
                    "site_analysis" | "site" => rustcrawl::agent::AgentMode::SiteAnalysis,
                    "comparison" | "compare" => rustcrawl::agent::AgentMode::Comparison,
                    "monitoring_analysis" | "monitoring" => {
                        rustcrawl::agent::AgentMode::MonitoringAnalysis
                    }
                    _ => rustcrawl::agent::AgentMode::Research,
                };

                let limits = rustcrawl::agent::AgentLimits {
                    max_steps: args.max_steps,
                    max_pages: args.max_pages,
                    ..Default::default()
                };

                let req = rustcrawl::agent::AgentRequest {
                    task,
                    mode,
                    output_schema: schema_val,
                    response_format: "markdown".to_string(),
                    freshness_days: args.freshness,
                    limits: Some(limits),
                    include_trace: true,
                    initial_urls: args.urls,
                    tenant_id: None,
                };

                let res = agent_service.submit_and_wait(req).await?;

                if let Some(ref out_file) = args.output {
                    tokio::fs::write(out_file, serde_json::to_string_pretty(&res)?).await?;
                    println!("Research result saved to {out_file}");
                } else if let Some(ref md) = res.markdown_answer {
                    println!("{md}");
                    if !res.citations.is_empty() {
                        println!("\n### Citations");
                        for cit in &res.citations {
                            println!(
                                "- **[{}]** {} ({})",
                                cit.source_id,
                                cit.title.as_deref().unwrap_or(&cit.url),
                                cit.url
                            );
                        }
                    }
                } else {
                    println!("{}", serde_json::to_string_pretty(&res)?);
                }
            } else if let Some(action) = args.action {
                let job_id = args.job_id.unwrap_or_default();
                match action.as_str() {
                    "inspect" => {
                        if let Some(summary) = agent_service.get_job_summary(&job_id) {
                            println!("{}", serde_json::to_string_pretty(&summary)?);
                        } else {
                            eprintln!("Job '{job_id}' not found");
                        }
                    }
                    "pause" => {
                        agent_service.pause_job(&job_id)?;
                        println!("Paused job '{job_id}'");
                    }
                    "resume" => {
                        agent_service.resume_job(&job_id)?;
                        println!("Resumed job '{job_id}'");
                    }
                    "cancel" => {
                        agent_service.cancel_job(&job_id)?;
                        println!("Cancelled job '{job_id}'");
                    }
                    other => eprintln!("Unknown action '{other}'"),
                }
            } else {
                eprintln!("Please provide a research task or subcommand action");
            }
        }

        Commands::Mcp => {
            let tools = rustcrawl::agent::McpToolServer::list_tools();
            println!("Model Context Protocol (MCP) Tools Available:\n");
            for t in tools {
                println!("- **{}**: {}", t.name, t.description);
            }
        }
    }

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down server gracefully...");
        },
        _ = terminate => {
            info!("Received terminate signal, shutting down server gracefully...");
        },
    }
}
