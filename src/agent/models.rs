use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Operational modes supported by the autonomous web research agent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    #[default]
    Research,
    StructuredResearch,
    SiteAnalysis,
    Comparison,
    MonitoringAnalysis,
}

impl AgentMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::StructuredResearch => "structured_research",
            Self::SiteAnalysis => "site_analysis",
            Self::Comparison => "comparison",
            Self::MonitoringAnalysis => "monitoring_analysis",
        }
    }
}

/// Lifecycle states of an agent research job
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    #[default]
    Queued,
    Planning,
    Researching,
    Evaluating,
    Synthesizing,
    Validating,
    Completed,
    Failed,
    Paused,
    Cancelled,
}

impl AgentStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Planning
                | Self::Researching
                | Self::Evaluating
                | Self::Synthesizing
                | Self::Validating
        )
    }
}

/// Status of individual research subgoals
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubgoalStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Skipped,
    Failed,
}

/// Deterministic source classifications for authority ranking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Official,
    Primary,
    Documentation,
    Repository,
    Government,
    Academic,
    News,
    Community,
    #[default]
    Unknown,
}

impl SourceType {
    /// Quality ranking score (0.0 - 1.0)
    pub fn quality_weight(&self) -> f32 {
        match self {
            Self::Official => 1.0,
            Self::Primary => 0.95,
            Self::Documentation => 0.90,
            Self::Repository => 0.85,
            Self::Academic => 0.85,
            Self::Government => 0.85,
            Self::News => 0.65,
            Self::Community => 0.50,
            Self::Unknown => 0.40,
        }
    }
}

/// Confidence rating for synthesized claims
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClaimConfidence {
    High,
    #[default]
    Medium,
    Low,
    Unverified,
}

/// User/client specified bounds for the agent research job
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentLimits {
    pub max_steps: Option<u32>,
    pub max_pages: Option<u32>,
    pub max_duration_seconds: Option<u64>,
    pub max_search_queries: Option<u32>,
    pub max_llm_calls: Option<u32>,
    pub max_browser_pages: Option<u32>,
}

/// Inbound request payload for initiating an autonomous research job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub task: String,
    #[serde(default)]
    pub mode: AgentMode,
    #[serde(default)]
    pub output_schema: Option<Value>,
    #[serde(default = "default_response_format")]
    pub response_format: String,
    #[serde(default)]
    pub freshness_days: Option<u32>,
    #[serde(default)]
    pub limits: Option<AgentLimits>,
    #[serde(default)]
    pub include_trace: bool,
    #[serde(default)]
    pub initial_urls: Vec<String>,
    #[serde(default)]
    pub tenant_id: Option<String>,
}

fn default_response_format() -> String {
    "markdown".to_string()
}

/// A decomposed subgoal in a research plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchSubgoal {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub status: SubgoalStatus,
    #[serde(default = "default_priority")]
    pub priority: u8,
    #[serde(default)]
    pub target_urls: Vec<String>,
    #[serde(default)]
    pub search_queries: Vec<String>,
    #[serde(default)]
    pub extracted_facts: Vec<String>,
}

fn default_priority() -> u8 {
    1
}

/// Bounded research plan generated deterministically or via LLM planner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchPlan {
    pub objective: String,
    pub subgoals: Vec<ResearchSubgoal>,
    #[serde(default)]
    pub expected_output: Option<Value>,
}

/// Concrete citation linking a synthesized claim or field to a verified evidence record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub source_id: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(default)]
    pub supports: Vec<String>,
}

/// An individual synthesized claim backed by evidence IDs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub id: String,
    pub text: String,
    pub evidence_ids: Vec<String>,
    pub confidence: ClaimConfidence,
}

/// Conflict recorded when multiple sources disagree on a field or factual assertion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceConflict {
    pub field: String,
    pub evidence_ids: Vec<String>,
    pub details: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_value: Option<Value>,
}

/// Research execution resource and performance accounting statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResearchStats {
    pub steps: u32,
    pub pages_examined: u32,
    pub sources_used: usize,
    pub search_queries: u32,
    pub browser_pages_used: u32,
    pub llm_calls: u32,
    pub duration_ms: u64,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
}

/// Overall confidence diagnostics for research completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfidence {
    pub overall: String,
    pub missing_fields: Vec<String>,
    pub conflicts: usize,
    pub score: f32,
}

/// High-level audit and trace event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTraceEvent {
    pub timestamp: DateTime<Utc>,
    pub step: u32,
    pub phase: String,
    pub action: String,
    pub details: Value,
}

/// Complete result returned upon agent research completion or partial execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub job_id: String,
    pub task: String,
    pub status: AgentStatus,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_answer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown_answer: Option<String>,
    pub research: ResearchStats,
    #[serde(default)]
    pub citations: Vec<Citation>,
    #[serde(default)]
    pub claims: Vec<Claim>,
    #[serde(default)]
    pub conflicts: Vec<EvidenceConflict>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<AgentConfidence>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<Vec<AgentTraceEvent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Progress details for polling `GET /v1/agent/{job_id}`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentProgress {
    pub steps: u32,
    pub max_steps: u32,
    pub sources_found: usize,
    pub sources_used: usize,
    pub pages_examined: u32,
    pub current_phase: String,
}

/// Job status summary for polling `GET /v1/agent/{job_id}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentJobSummary {
    pub job_id: String,
    pub task: String,
    pub status: AgentStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub progress: AgentProgress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<AgentResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
