use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    #[default]
    Standalone,
    Distributed,
    Worker,
}

impl FromStr for ExecutionMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "standalone" => Ok(ExecutionMode::Standalone),
            "distributed" => Ok(ExecutionMode::Distributed),
            "worker" => Ok(ExecutionMode::Worker),
            _ => Err(format!("Unknown execution mode: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum JobType {
    Scrape,
    Crawl,
    Map,
    Search,
    Extract,
    IncrementalCrawl,
    Agent,
}

impl fmt::Display for JobType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JobType::Scrape => write!(f, "scrape"),
            JobType::Crawl => write!(f, "crawl"),
            JobType::Map => write!(f, "map"),
            JobType::Search => write!(f, "search"),
            JobType::Extract => write!(f, "extract"),
            JobType::IncrementalCrawl => write!(f, "incremental_crawl"),
            JobType::Agent => write!(f, "agent"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl fmt::Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JobStatus::Queued => write!(f, "queued"),
            JobStatus::Running => write!(f, "running"),
            JobStatus::Paused => write!(f, "paused"),
            JobStatus::Completed => write!(f, "completed"),
            JobStatus::Failed => write!(f, "failed"),
            JobStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    HttpFetch,
    BrowserRender,
    DocumentParse,
    Extract,
    SitemapParse,
    Search,
    IndexChunk,
    EmbedChunk,
    AgentTask,
}

impl fmt::Display for TaskType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskType::HttpFetch => write!(f, "http_fetch"),
            TaskType::BrowserRender => write!(f, "browser_render"),
            TaskType::DocumentParse => write!(f, "document_parse"),
            TaskType::Extract => write!(f, "extract"),
            TaskType::SitemapParse => write!(f, "sitemap_parse"),
            TaskType::Search => write!(f, "search"),
            TaskType::IndexChunk => write!(f, "index_chunk"),
            TaskType::EmbedChunk => write!(f, "embed_chunk"),
            TaskType::AgentTask => write!(f, "agent_task"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Leased,
    Running,
    Completed,
    Failed,
    DeadLetter,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub enum TaskPriority {
    Critical = 0,
    High = 1,
    #[default]
    Normal = 2,
    Low = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum WorkerType {
    Http,
    Browser,
    Document,
    Extraction,
    Indexing,
    Embedding,
    Agent,
    General,
}

impl FromStr for WorkerType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "http" => Ok(WorkerType::Http),
            "browser" => Ok(WorkerType::Browser),
            "document" => Ok(WorkerType::Document),
            "extraction" => Ok(WorkerType::Extraction),
            "indexing" => Ok(WorkerType::Indexing),
            "embedding" => Ok(WorkerType::Embedding),
            "agent" => Ok(WorkerType::Agent),
            "general" => Ok(WorkerType::General),
            _ => Err(format!("Unknown worker type: {s}")),
        }
    }
}

impl WorkerType {
    pub fn can_handle(&self, task_type: TaskType) -> bool {
        match self {
            WorkerType::General => true,
            WorkerType::Http => matches!(
                task_type,
                TaskType::HttpFetch | TaskType::SitemapParse | TaskType::Search
            ),
            WorkerType::Browser => matches!(task_type, TaskType::BrowserRender),
            WorkerType::Document => matches!(task_type, TaskType::DocumentParse),
            WorkerType::Extraction => matches!(task_type, TaskType::Extract),
            WorkerType::Indexing => matches!(task_type, TaskType::IndexChunk),
            WorkerType::Embedding => matches!(task_type, TaskType::EmbedChunk),
            WorkerType::Agent => matches!(task_type, TaskType::AgentTask),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerCapability {
    pub worker_type: WorkerType,
    pub max_concurrency: usize,
    pub browser_available: bool,
    pub llm_available: bool,
    pub supported_tasks: Vec<TaskType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerInfo {
    pub worker_id: String,
    pub worker_type: WorkerType,
    pub hostname: String,
    pub version: String,
    pub max_concurrency: usize,
    pub active_tasks: usize,
    pub browser_available: bool,
    pub llm_available: bool,
    pub started_at: u64,
    pub last_heartbeat: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: String,
    pub job_type: JobType,
    pub status: JobStatus,
    pub configuration: serde_json::Value,
    pub tenant_id: Option<String>,
    pub created_at: u64,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub progress: JobProgressSummary,
    pub error_summary: Option<String>,
    pub result_location: Option<String>,
}

impl JobRecord {
    pub fn new(
        id: impl Into<String>,
        job_type: JobType,
        configuration: serde_json::Value,
        tenant_id: Option<String>,
    ) -> Self {
        let now = Utc::now().timestamp() as u64;
        Self {
            id: id.into(),
            job_type,
            status: JobStatus::Queued,
            configuration,
            tenant_id,
            created_at: now,
            started_at: None,
            completed_at: None,
            progress: JobProgressSummary::default(),
            error_summary: None,
            result_location: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobProgressSummary {
    pub total_discovered: usize,
    pub queued_tasks: usize,
    pub active_tasks: usize,
    pub completed_pages: usize,
    pub failed_pages: usize,
    pub unchanged_pages: usize,
    pub bytes_downloaded: usize,
    pub duration_seconds: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobUpdate {
    pub status: Option<JobStatus>,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub progress: Option<JobProgressSummary>,
    pub error_summary: Option<String>,
    pub result_location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub task_id: String,
    pub job_id: String,
    pub task_type: TaskType,
    pub payload: serde_json::Value,
    pub priority: TaskPriority,
    pub attempt: usize,
    pub max_attempts: usize,
    pub status: TaskStatus,
    pub lease_owner: Option<String>,
    pub lease_expiry: Option<u64>,
    pub created_at: u64,
    pub available_at: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedTask {
    pub task_id: String,
    pub job_id: String,
    pub task_type: TaskType,
    pub payload: serde_json::Value,
    pub priority: TaskPriority,
    pub max_attempts: usize,
    pub available_at: u64,
}

impl QueuedTask {
    pub fn new(
        job_id: impl Into<String>,
        task_type: TaskType,
        payload: serde_json::Value,
        priority: TaskPriority,
        max_attempts: usize,
    ) -> Self {
        let now = Utc::now().timestamp() as u64;
        Self {
            task_id: Uuid::new_v4().to_string(),
            job_id: job_id.into(),
            task_type,
            payload,
            priority,
            max_attempts,
            available_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskLease {
    pub lease_id: String,
    pub task_id: String,
    pub job_id: String,
    pub worker_id: String,
    pub task_type: TaskType,
    pub payload: serde_json::Value,
    pub attempt: usize,
    pub max_attempts: usize,
    pub lease_expiry: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskFailure {
    pub error_message: String,
    pub is_retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEnvelope {
    pub version: u16,
    pub task_id: String,
    pub job_id: String,
    pub task_type: TaskType,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStats {
    pub queued_total: usize,
    pub queued_by_type: HashMap<TaskType, usize>,
    pub leased_total: usize,
    pub completed_total: usize,
    pub failed_total: usize,
    pub dlq_total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemEventType {
    JobStarted,
    JobProgress,
    PageCompleted,
    ChangeDetected,
    JobCompleted,
    JobFailed,
    JobCancelled,
    JobPaused,
    JobResumed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEventRecord {
    pub event_id: String,
    pub job_id: String,
    pub event_type: SystemEventType,
    pub timestamp: u64,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    pub id: String,
    pub url: String,
    pub secret: Option<String>,
    pub events: Vec<SystemEventType>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDeliveryRecord {
    pub delivery_id: String,
    pub event_id: String,
    pub job_id: String,
    pub endpoint_url: String,
    pub event_type: SystemEventType,
    pub status: WebhookDeliveryStatus,
    pub attempt: usize,
    pub response_status: Option<u16>,
    pub error: Option<String>,
    pub next_retry_at: Option<u64>,
    pub created_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookDeliveryStatus {
    Pending,
    Delivered,
    Retrying,
    Failed,
}
