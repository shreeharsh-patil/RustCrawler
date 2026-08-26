use std::sync::Arc;

use crate::agent::evidence::EvidenceStore;
use crate::agent::models::{AgentRequest, ResearchPlan};
use crate::agent::tools::{AgentToolCall, SearchToolInput};
use crate::config::Config;

/// Evaluation outcome from inspecting evidence coverage
#[derive(Debug, Clone)]
pub struct EvaluationResult {
    pub is_sufficient: bool,
    pub missing_fields: Vec<String>,
    pub conflicts_detected: usize,
    pub followup_calls: Vec<AgentToolCall>,
    pub notes: String,
}

/// Evaluator identifying evidence gaps, missing schema attributes, and formulating bounded follow-up queries
pub struct EvidenceGapEvaluator {
    config: Arc<Config>,
}

impl EvidenceGapEvaluator {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    /// Evaluates current evidence store against research plan and optional schema
    pub fn evaluate(
        &self,
        req: &AgentRequest,
        plan: &ResearchPlan,
        store: &EvidenceStore,
        current_round: u32,
    ) -> EvaluationResult {
        let mut missing_fields = Vec::new();
        let mut followup_calls = Vec::new();
        let all_evidence = store.all();
        let conflicts = store.conflicts();

        // 1. Check if we have collected any evidence at all
        if all_evidence.is_empty() {
            // Need broad initial discovery
            for subgoal in &plan.subgoals {
                for q in &subgoal.search_queries {
                    followup_calls.push(AgentToolCall::Search(SearchToolInput {
                        query: q.clone(),
                        limit: Some(5),
                        freshness_days: req.freshness_days,
                    }));
                }
            }

            return EvaluationResult {
                is_sufficient: false,
                missing_fields: vec!["all_evidence".to_string()],
                conflicts_detected: 0,
                followup_calls,
                notes: "Initial research round required".to_string(),
            };
        }

        // 2. Check schema fields if structured extraction was requested
        if let Some(ref schema) = req.output_schema {
            if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
                for (field_name, _) in props {
                    let has_field_evidence = all_evidence.iter().any(|e| {
                        let text_matches = e
                            .excerpt
                            .to_lowercase()
                            .contains(&field_name.to_lowercase());
                        let fact_matches = e
                            .extracted_fact
                            .as_ref()
                            .and_then(|f| f.get(field_name))
                            .map(|v| !v.is_null())
                            .unwrap_or(false);
                        text_matches || fact_matches
                    });

                    if !has_field_evidence {
                        missing_fields.push(field_name.clone());
                        if current_round < self.config.agent_max_research_rounds {
                            followup_calls.push(AgentToolCall::Search(SearchToolInput {
                                query: format!("{} {}", req.task, field_name),
                                limit: Some(3),
                                freshness_days: req.freshness_days,
                            }));
                        }
                    }
                }
            }
        }

        // 3. Source diversity check
        let unique_sources = store.unique_sources_count();
        let meets_diversity = unique_sources >= self.config.agent_min_source_diversity;
        if !meets_diversity && current_round < self.config.agent_max_research_rounds {
            followup_calls.push(AgentToolCall::Search(SearchToolInput {
                query: format!("{} alternative sources", req.task),
                limit: Some(3),
                freshness_days: req.freshness_days,
            }));
        }

        // Check if we can terminate or need another targeted round
        let is_sufficient = (missing_fields.is_empty() && meets_diversity)
            || current_round >= self.config.agent_max_research_rounds;

        let notes = format!(
            "Round {}/{}: {} evidence items from {} sources. Missing fields: {:?}. Conflicts: {}.",
            current_round,
            self.config.agent_max_research_rounds,
            all_evidence.len(),
            unique_sources,
            missing_fields,
            conflicts.len()
        );

        EvaluationResult {
            is_sufficient,
            missing_fields,
            conflicts_detected: conflicts.len(),
            followup_calls,
            notes,
        }
    }
}
