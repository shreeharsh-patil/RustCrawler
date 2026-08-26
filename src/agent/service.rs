use std::sync::Arc;

use chrono::Utc;
use dashmap::DashMap;
use uuid::Uuid;

use crate::agent::budgets::AgentBudget;
use crate::agent::evaluator::EvidenceGapEvaluator;
use crate::agent::executor::AgentExecutor;
use crate::agent::models::{AgentJobSummary, AgentRequest, AgentResult};
use crate::agent::planner::AgentPlanner;
use crate::agent::state::AgentState;
use crate::agent::synthesis::ResearchSynthesizer;
use crate::agent::tools::AgentToolExecutor;
use crate::config::Config;
use crate::crawl::CrawlerService;
use crate::discovery::service::MapService;
use crate::distributed::EventBus;
use crate::error::CrawlerError;
use crate::extraction::llm::LlmProvider;
use crate::extraction::service::ExtractionService;
use crate::knowledge::KnowledgeEngine;
use crate::search::service::SearchService;
use crate::service::ScraperService;

/// Central service orchestrating autonomous agent research jobs, background execution, and lifecycle controls
pub struct AgentService {
    config: Arc<Config>,
    tool_executor: Arc<AgentToolExecutor>,
    executor: Arc<AgentExecutor>,
    active_states: DashMap<String, Arc<AgentState>>,
    completed_jobs: DashMap<String, AgentJobSummary>,
    event_bus: Option<Arc<EventBus>>,
}

impl AgentService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: Arc<Config>,
        search_service: Option<Arc<SearchService>>,
        map_service: Option<Arc<MapService>>,
        scraper_service: Arc<ScraperService>,
        crawler_service: Arc<CrawlerService>,
        extraction_service: Arc<ExtractionService>,
        knowledge_engine: Option<Arc<KnowledgeEngine>>,
        llm_provider: Option<Arc<dyn LlmProvider>>,
        event_bus: Option<Arc<EventBus>>,
    ) -> Self {
        let tool_executor = Arc::new(AgentToolExecutor::new(
            config.clone(),
            search_service,
            map_service,
            scraper_service,
            crawler_service,
            extraction_service,
            knowledge_engine,
        ));

        let planner = Arc::new(AgentPlanner::new(config.clone(), llm_provider.clone()));
        let evaluator = Arc::new(EvidenceGapEvaluator::new(config.clone()));
        let synthesizer = Arc::new(ResearchSynthesizer::new(config.clone(), llm_provider));

        let executor = Arc::new(AgentExecutor::new(
            config.clone(),
            tool_executor.clone(),
            planner,
            evaluator,
            synthesizer,
        ));

        Self {
            config,
            tool_executor,
            executor,
            active_states: DashMap::new(),
            completed_jobs: DashMap::new(),
            event_bus,
        }
    }

    pub fn tool_executor(&self) -> &AgentToolExecutor {
        &self.tool_executor
    }

    /// Submits a research job and returns the assigned Job ID immediately (runs in background)
    pub async fn submit_job(&self, req: AgentRequest) -> Result<String, CrawlerError> {
        let job_id = format!("agent_{}", Uuid::new_v4());
        let budget = AgentBudget::from_config_and_limits(&self.config, req.limits.as_ref());
        let state = Arc::new(AgentState::new(job_id.clone(), budget));

        self.active_states.insert(job_id.clone(), state.clone());

        let executor = self.executor.clone();
        let states_map = self.active_states.clone();
        let completed_map = self.completed_jobs.clone();
        let task_name = req.task.clone();
        let job_id_clone = job_id.clone();
        let event_bus = self.event_bus.clone();

        tokio::spawn(async move {
            let result = executor.run_job(req, state.clone()).await;

            let summary = AgentJobSummary {
                job_id: job_id_clone.clone(),
                task: task_name,
                status: result.status,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                progress: state.get_progress(),
                result: Some(result),
                error: None,
            };

            completed_map.insert(job_id_clone.clone(), summary);
            states_map.remove(&job_id_clone);

            if let Some(bus) = event_bus {
                let event = crate::distributed::models::SystemEventRecord {
                    event_id: uuid::Uuid::new_v4().to_string(),
                    job_id: job_id_clone.clone(),
                    event_type: crate::distributed::models::SystemEventType::JobCompleted,
                    timestamp: Utc::now().timestamp_millis() as u64,
                    data: serde_json::json!({ "job_id": job_id_clone, "status": "completed" }),
                };
                bus.publish(event);
            }
        });

        Ok(job_id)
    }

    /// Submits a research job and awaits execution to completion
    pub async fn submit_and_wait(&self, req: AgentRequest) -> Result<AgentResult, CrawlerError> {
        let job_id = format!("agent_{}", Uuid::new_v4());
        let budget = AgentBudget::from_config_and_limits(&self.config, req.limits.as_ref());
        let state = Arc::new(AgentState::new(job_id.clone(), budget));

        self.active_states.insert(job_id.clone(), state.clone());

        let result = self.executor.run_job(req.clone(), state.clone()).await;

        let summary = AgentJobSummary {
            job_id: job_id.clone(),
            task: req.task,
            status: result.status,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            progress: state.get_progress(),
            result: Some(result.clone()),
            error: None,
        };

        self.completed_jobs.insert(job_id.clone(), summary);
        self.active_states.remove(&job_id);

        Ok(result)
    }

    /// Gets summary details and live progress for a research job
    pub fn get_job_summary(&self, job_id: &str) -> Option<AgentJobSummary> {
        if let Some(state) = self.active_states.get(job_id) {
            let plan_obj = state
                .plan
                .read()
                .ok()
                .and_then(|p| p.as_ref().map(|x| x.objective.clone()));
            Some(AgentJobSummary {
                job_id: job_id.to_string(),
                task: plan_obj.unwrap_or_else(|| "Autonomous Research".to_string()),
                status: state.get_status(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                progress: state.get_progress(),
                result: None,
                error: None,
            })
        } else {
            self.completed_jobs.get(job_id).map(|j| j.clone())
        }
    }

    /// Pauses an active research job
    pub fn pause_job(&self, job_id: &str) -> Result<(), CrawlerError> {
        if let Some(state) = self.active_states.get(job_id) {
            state.pause();
            Ok(())
        } else {
            Err(CrawlerError::JobNotFound(job_id.to_string()))
        }
    }

    /// Resumes a paused research job
    pub fn resume_job(&self, job_id: &str) -> Result<(), CrawlerError> {
        if let Some(state) = self.active_states.get(job_id) {
            state.resume();
            Ok(())
        } else {
            Err(CrawlerError::JobNotFound(job_id.to_string()))
        }
    }

    /// Cancels an active research job
    pub fn cancel_job(&self, job_id: &str) -> Result<(), CrawlerError> {
        if let Some(state) = self.active_states.get(job_id) {
            state.cancel();
            Ok(())
        } else {
            Err(CrawlerError::JobNotFound(job_id.to_string()))
        }
    }
}
