use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::agent::models::{AgentLimits, ResearchStats};
use crate::config::Config;
use crate::error::CrawlerError;

/// Bounded limits configured for a specific research execution run
#[derive(Debug, Clone)]
pub struct AgentBudget {
    pub max_steps: u32,
    pub max_pages: u32,
    pub max_search_queries: u32,
    pub max_query_variants_per_subgoal: u32,
    pub max_research_rounds: u32,
    pub max_browser_pages: u32,
    pub max_browser_actions_total: u32,
    pub max_llm_calls: u32,
    pub max_duration: Duration,
    pub deadline: Instant,
}

impl AgentBudget {
    pub fn from_config_and_limits(config: &Config, limits: Option<&AgentLimits>) -> Self {
        let max_steps = limits
            .and_then(|l| l.max_steps)
            .unwrap_or(config.agent_max_steps)
            .min(config.agent_max_steps);

        let max_pages = limits
            .and_then(|l| l.max_pages)
            .unwrap_or(config.agent_max_pages)
            .min(config.agent_max_pages);

        let max_search_queries = limits
            .and_then(|l| l.max_search_queries)
            .unwrap_or(config.agent_max_search_queries)
            .min(config.agent_max_search_queries);

        let max_browser_pages = limits
            .and_then(|l| l.max_browser_pages)
            .unwrap_or(config.agent_max_browser_pages)
            .min(config.agent_max_browser_pages);

        let max_llm_calls = limits
            .and_then(|l| l.max_llm_calls)
            .unwrap_or(config.agent_max_llm_calls)
            .min(config.agent_max_llm_calls);

        let duration_secs = limits
            .and_then(|l| l.max_duration_seconds)
            .unwrap_or(config.agent_max_duration_seconds)
            .min(config.agent_max_duration_seconds);

        let max_duration = Duration::from_secs(duration_secs);
        let deadline = Instant::now() + max_duration;

        Self {
            max_steps,
            max_pages,
            max_search_queries,
            max_query_variants_per_subgoal: config.agent_max_query_variants_per_subgoal,
            max_research_rounds: config.agent_max_research_rounds,
            max_browser_pages,
            max_browser_actions_total: config.agent_max_browser_actions_total,
            max_llm_calls,
            max_duration,
            deadline,
        }
    }
}

/// Thread-safe resource accounting tracker during research execution
pub struct BudgetTracker {
    pub current_steps: AtomicU32,
    pub current_pages: AtomicU32,
    pub current_search_queries: AtomicU32,
    pub current_browser_pages: AtomicU32,
    pub current_browser_actions: AtomicU32,
    pub current_llm_calls: AtomicU32,
    pub prompt_tokens: AtomicUsize,
    pub completion_tokens: AtomicUsize,
    pub start_time: Instant,
}

impl Default for BudgetTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl BudgetTracker {
    pub fn new() -> Self {
        Self {
            current_steps: AtomicU32::new(0),
            current_pages: AtomicU32::new(0),
            current_search_queries: AtomicU32::new(0),
            current_browser_pages: AtomicU32::new(0),
            current_browser_actions: AtomicU32::new(0),
            current_llm_calls: AtomicU32::new(0),
            prompt_tokens: AtomicUsize::new(0),
            completion_tokens: AtomicUsize::new(0),
            start_time: Instant::now(),
        }
    }

    pub fn check_deadline(&self, budget: &AgentBudget) -> Result<(), CrawlerError> {
        if Instant::now() > budget.deadline {
            return Err(CrawlerError::AgentTimeLimitReached(
                budget.max_duration.as_secs(),
            ));
        }
        Ok(())
    }

    pub fn record_step(&self, budget: &AgentBudget) -> Result<u32, CrawlerError> {
        self.check_deadline(budget)?;
        let prev = self.current_steps.fetch_add(1, Ordering::SeqCst);
        if prev >= budget.max_steps {
            return Err(CrawlerError::AgentStepLimitReached(budget.max_steps));
        }
        Ok(prev + 1)
    }

    pub fn record_page(&self, budget: &AgentBudget) -> Result<u32, CrawlerError> {
        self.check_deadline(budget)?;
        let prev = self.current_pages.fetch_add(1, Ordering::SeqCst);
        if prev >= budget.max_pages {
            return Err(CrawlerError::AgentPageLimitReached(budget.max_pages));
        }
        Ok(prev + 1)
    }

    pub fn record_search_query(&self, budget: &AgentBudget) -> Result<u32, CrawlerError> {
        self.check_deadline(budget)?;
        let prev = self.current_search_queries.fetch_add(1, Ordering::SeqCst);
        if prev >= budget.max_search_queries {
            return Err(CrawlerError::SearchLimitExceeded(format!(
                "Search query budget of {} exceeded",
                budget.max_search_queries
            )));
        }
        Ok(prev + 1)
    }

    pub fn record_browser_page(&self, budget: &AgentBudget) -> Result<u32, CrawlerError> {
        self.check_deadline(budget)?;
        let prev = self.current_browser_pages.fetch_add(1, Ordering::SeqCst);
        if prev >= budget.max_browser_pages {
            return Err(CrawlerError::AgentPageLimitReached(
                budget.max_browser_pages,
            ));
        }
        Ok(prev + 1)
    }

    pub fn record_browser_action(&self, budget: &AgentBudget) -> Result<u32, CrawlerError> {
        self.check_deadline(budget)?;
        let prev = self.current_browser_actions.fetch_add(1, Ordering::SeqCst);
        if prev >= budget.max_browser_actions_total {
            return Err(CrawlerError::ActionFailed(
                "browser_action".to_string(),
                format!(
                    "Browser action limit of {} reached",
                    budget.max_browser_actions_total
                ),
            ));
        }
        Ok(prev + 1)
    }

    pub fn record_llm_call(&self, budget: &AgentBudget) -> Result<u32, CrawlerError> {
        self.check_deadline(budget)?;
        let prev = self.current_llm_calls.fetch_add(1, Ordering::SeqCst);
        if prev >= budget.max_llm_calls {
            return Err(CrawlerError::AgentLlmLimitReached(budget.max_llm_calls));
        }
        Ok(prev + 1)
    }

    pub fn record_tokens(&self, prompt: usize, completion: usize) {
        self.prompt_tokens.fetch_add(prompt, Ordering::Relaxed);
        self.completion_tokens
            .fetch_add(completion, Ordering::Relaxed);
    }

    pub fn stats(&self, sources_used: usize) -> ResearchStats {
        ResearchStats {
            steps: self.current_steps.load(Ordering::Relaxed),
            pages_examined: self.current_pages.load(Ordering::Relaxed),
            sources_used,
            search_queries: self.current_search_queries.load(Ordering::Relaxed),
            browser_pages_used: self.current_browser_pages.load(Ordering::Relaxed),
            llm_calls: self.current_llm_calls.load(Ordering::Relaxed),
            duration_ms: self.start_time.elapsed().as_millis() as u64,
            prompt_tokens: self.prompt_tokens.load(Ordering::Relaxed),
            completion_tokens: self.completion_tokens.load(Ordering::Relaxed),
        }
    }
}
