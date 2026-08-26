use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use chrono::Utc;
use serde_json::Value;

use crate::agent::budgets::{AgentBudget, BudgetTracker};
use crate::agent::evidence::EvidenceStore;
use crate::agent::models::{AgentProgress, AgentStatus, AgentTraceEvent, ResearchPlan};

/// State container holding the mutable execution state of an agent research job
pub struct AgentState {
    pub job_id: String,
    pub status: RwLock<AgentStatus>,
    pub budget: AgentBudget,
    pub tracker: Arc<BudgetTracker>,
    pub evidence: Arc<EvidenceStore>,
    pub plan: RwLock<Option<ResearchPlan>>,
    pub trace_events: RwLock<Vec<AgentTraceEvent>>,
    pub is_paused: AtomicBool,
    pub is_cancelled: AtomicBool,
    pub current_phase: RwLock<String>,
}

impl AgentState {
    pub fn new(job_id: String, budget: AgentBudget) -> Self {
        Self {
            job_id,
            status: RwLock::new(AgentStatus::Queued),
            budget,
            tracker: Arc::new(BudgetTracker::new()),
            evidence: Arc::new(EvidenceStore::new()),
            plan: RwLock::new(None),
            trace_events: RwLock::new(Vec::new()),
            is_paused: AtomicBool::new(false),
            is_cancelled: AtomicBool::new(false),
            current_phase: RwLock::new("Queued".to_string()),
        }
    }

    pub fn set_status(&self, status: AgentStatus) {
        if let Ok(mut s) = self.status.write() {
            *s = status;
        }
    }

    pub fn get_status(&self) -> AgentStatus {
        self.status
            .read()
            .map(|s| *s)
            .unwrap_or(AgentStatus::Failed)
    }

    pub fn set_phase(&self, phase: &str) {
        if let Ok(mut p) = self.current_phase.write() {
            *p = phase.to_string();
        }
    }

    pub fn add_trace(&self, phase: &str, action: &str, details: Value) {
        let step = self.tracker.current_steps.load(Ordering::Relaxed);
        let event = AgentTraceEvent {
            timestamp: Utc::now(),
            step,
            phase: phase.to_string(),
            action: action.to_string(),
            details,
        };

        if let Ok(mut events) = self.trace_events.write() {
            events.push(event);
        }
    }

    pub fn get_progress(&self) -> AgentProgress {
        let steps = self.tracker.current_steps.load(Ordering::Relaxed);
        let pages_examined = self.tracker.current_pages.load(Ordering::Relaxed);
        let sources_found = self.evidence.all().len();
        let sources_used = self.evidence.unique_sources_count();
        let current_phase = self
            .current_phase
            .read()
            .map(|p| p.clone())
            .unwrap_or_else(|_| "Unknown".to_string());

        AgentProgress {
            steps,
            max_steps: self.budget.max_steps,
            sources_found,
            sources_used,
            pages_examined,
            current_phase,
        }
    }

    pub fn pause(&self) {
        self.is_paused.store(true, Ordering::SeqCst);
        self.set_status(AgentStatus::Paused);
    }

    pub fn resume(&self) {
        self.is_paused.store(false, Ordering::SeqCst);
        self.set_status(AgentStatus::Researching);
    }

    pub fn cancel(&self) {
        self.is_cancelled.store(true, Ordering::SeqCst);
        self.set_status(AgentStatus::Cancelled);
    }

    pub fn is_active(&self) -> bool {
        !self.is_cancelled.load(Ordering::SeqCst) && !self.is_paused.load(Ordering::SeqCst)
    }
}
