use std::sync::Arc;

use chrono::Utc;
use serde_json::json;

use crate::agent::citations::CitationValidator;
use crate::agent::evaluator::EvidenceGapEvaluator;
use crate::agent::evidence::Evidence;
use crate::agent::models::{AgentRequest, AgentResult, AgentStatus};
use crate::agent::planner::AgentPlanner;
use crate::agent::policy::AgentPolicy;
use crate::agent::state::AgentState;
use crate::agent::synthesis::ResearchSynthesizer;
use crate::agent::tools::{
    AgentToolCall, AgentToolExecutor, AgentToolResult, ScrapeToolInput, SearchToolInput,
};
use crate::config::Config;
use crate::error::CrawlerError;
use crate::knowledge::chunking::dedupe::compute_chunk_hash;

/// Central executor running the state machine loop of an autonomous research job
pub struct AgentExecutor {
    config: Arc<Config>,
    tool_executor: Arc<AgentToolExecutor>,
    planner: Arc<AgentPlanner>,
    evaluator: Arc<EvidenceGapEvaluator>,
    synthesizer: Arc<ResearchSynthesizer>,
}

impl AgentExecutor {
    pub fn new(
        config: Arc<Config>,
        tool_executor: Arc<AgentToolExecutor>,
        planner: Arc<AgentPlanner>,
        evaluator: Arc<EvidenceGapEvaluator>,
        synthesizer: Arc<ResearchSynthesizer>,
    ) -> Self {
        Self {
            config,
            tool_executor,
            planner,
            evaluator,
            synthesizer,
        }
    }

    /// Executes the complete research job state machine
    pub async fn run_job(&self, req: AgentRequest, state: Arc<AgentState>) -> AgentResult {
        let job_id = state.job_id.clone();
        let task = req.task.clone();

        // 1. Planning Phase
        state.set_status(AgentStatus::Planning);
        state.set_phase("Planning");
        state.add_trace("planning", "plan_started", json!({ "task": task }));

        let plan = match self.planner.create_plan(&req).await {
            Ok(p) => {
                if let Ok(mut plan_slot) = state.plan.write() {
                    *plan_slot = Some(p.clone());
                }
                state.add_trace(
                    "planning",
                    "plan_created",
                    json!({
                        "objective": p.objective,
                        "subgoals_count": p.subgoals.len()
                    }),
                );
                p
            }
            Err(e) => {
                state.set_status(AgentStatus::Failed);
                return self.build_error_result(job_id, task, state, e.to_string());
            }
        };

        // 2. Research Phase
        state.set_status(AgentStatus::Researching);
        state.set_phase("Researching");

        let mut research_round = 1;
        let mut warnings = Vec::new();

        // Initial subgoal execution
        for subgoal in &plan.subgoals {
            if !state.is_active() {
                break;
            }

            if let Err(e) = state.tracker.record_step(&state.budget) {
                warnings.push(format!("Step budget reached: {e}"));
                break;
            }

            state.add_trace(
                "researching",
                "subgoal_started",
                json!({
                    "subgoal_id": subgoal.id,
                    "description": subgoal.description
                }),
            );

            // A. Target direct URLs if specified in subgoal or request
            for url in &subgoal.target_urls {
                if !state.is_active() {
                    break;
                }
                if state.evidence.has_visited_url(url) {
                    continue;
                }

                if let Err(e) = self.scrape_and_extract_evidence(url, None, &state).await {
                    warnings.push(format!("Failed to scrape target URL {url}: {e}"));
                }
            }

            // B. Execute search queries
            for query in &subgoal.search_queries {
                if !state.is_active() {
                    break;
                }

                if let Err(e) = state.tracker.record_search_query(&state.budget) {
                    warnings.push(format!("Search query limit reached: {e}"));
                    break;
                }

                state.add_trace("researching", "search_executed", json!({ "query": query }));

                let search_call = AgentToolCall::Search(SearchToolInput {
                    query: query.clone(),
                    limit: Some(5),
                    freshness_days: req.freshness_days,
                });

                if let Ok(AgentToolResult::Search { results, .. }) =
                    self.tool_executor.execute(search_call).await
                {
                    // Sort discovered links by source quality
                    let mut ranked = results;
                    ranked.sort_by(|a, b| {
                        let type_a = AgentPolicy::classify_source(&a.url, a.title.as_deref());
                        let type_b = AgentPolicy::classify_source(&b.url, b.title.as_deref());
                        type_b
                            .quality_weight()
                            .partial_cmp(&type_a.quality_weight())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });

                    // Scrape top candidate sources (up to 3 per search query)
                    for link in ranked.into_iter().take(3) {
                        if !state.is_active() {
                            break;
                        }
                        if state.evidence.has_visited_url(&link.url) {
                            continue;
                        }

                        if let Err(e) = self
                            .scrape_and_extract_evidence(&link.url, link.title.as_deref(), &state)
                            .await
                        {
                            warnings
                                .push(format!("Failed to scrape search result {}: {e}", link.url));
                        }
                    }
                }
            }
        }

        // 3. Evaluation and Iterative Gap-Filling Phase
        state.set_status(AgentStatus::Evaluating);
        state.set_phase("Evaluating");

        while research_round <= self.config.agent_max_research_rounds && state.is_active() {
            let eval = self
                .evaluator
                .evaluate(&req, &plan, &state.evidence, research_round);
            state.add_trace(
                "evaluating",
                "round_evaluation",
                json!({
                    "round": research_round,
                    "is_sufficient": eval.is_sufficient,
                    "missing_fields": eval.missing_fields,
                    "conflicts": eval.conflicts_detected
                }),
            );

            if eval.is_sufficient || eval.followup_calls.is_empty() {
                break;
            }

            // Execute follow-up research calls
            for call in eval.followup_calls {
                if !state.is_active() {
                    break;
                }

                if let Err(e) = state.tracker.record_step(&state.budget) {
                    warnings.push(format!("Step budget reached during follow-up: {e}"));
                    break;
                }

                if let AgentToolCall::Search(ref _s_input) = call {
                    if let Err(e) = state.tracker.record_search_query(&state.budget) {
                        warnings.push(format!("Search limit reached: {e}"));
                        break;
                    }

                    if let Ok(AgentToolResult::Search { results, .. }) =
                        self.tool_executor.execute(call).await
                    {
                        for link in results.into_iter().take(2) {
                            if !state.evidence.has_visited_url(&link.url) {
                                let _ = self
                                    .scrape_and_extract_evidence(
                                        &link.url,
                                        link.title.as_deref(),
                                        &state,
                                    )
                                    .await;
                            }
                        }
                    }
                }
            }

            research_round += 1;
        }

        // Check if paused or cancelled
        if state.is_cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            state.set_status(AgentStatus::Cancelled);
            return self.build_cancelled_result(job_id, task, state, warnings);
        }

        // 4. Synthesis Phase
        state.set_status(AgentStatus::Synthesizing);
        state.set_phase("Synthesizing");
        state.add_trace("synthesizing", "synthesis_started", json!({}));

        let synthesis_out = match self
            .synthesizer
            .synthesize(&req, &plan, &state.evidence)
            .await
        {
            Ok(out) => out,
            Err(e) => {
                warnings.push(format!("LLM synthesis fallback: {e}"));
                self.synthesizer
                    .synthesize_deterministic(&req, &state.evidence)
            }
        };

        // 5. Citations and Validation Phase
        state.set_status(AgentStatus::Validating);
        state.set_phase("Validating");

        let mut claims = synthesis_out.claims;
        let md_content = synthesis_out.markdown_answer.clone().unwrap_or_default();
        let (citations, cit_warnings) = CitationValidator::validate_and_build_citations(
            &md_content,
            &mut claims,
            &state.evidence,
        );

        warnings.extend(cit_warnings);

        state.set_status(AgentStatus::Completed);
        state.set_phase("Completed");
        state.add_trace(
            "completed",
            "job_finished",
            json!({
                "citations_count": citations.len(),
                "claims_count": claims.len()
            }),
        );

        let stats = state.tracker.stats(state.evidence.unique_sources_count());
        let trace = if req.include_trace {
            state.trace_events.read().ok().map(|t| t.clone())
        } else {
            None
        };

        AgentResult {
            job_id,
            task,
            status: AgentStatus::Completed,
            success: true,
            data: synthesis_out.data,
            text_answer: synthesis_out.text_answer,
            markdown_answer: synthesis_out.markdown_answer,
            research: stats,
            citations,
            claims,
            conflicts: state.evidence.conflicts(),
            confidence: Some(synthesis_out.confidence),
            warnings,
            trace,
            error: None,
        }
    }

    async fn scrape_and_extract_evidence(
        &self,
        url: &str,
        suggested_title: Option<&str>,
        state: &AgentState,
    ) -> Result<(), CrawlerError> {
        // Enforce page budget limit
        state.tracker.record_page(&state.budget)?;
        state.evidence.record_visited_url(url);

        state.add_trace("researching", "scrape_page", json!({ "url": url }));

        let scrape_call = AgentToolCall::Scrape(ScrapeToolInput {
            url: url.to_string(),
            formats: None,
            only_main_content: Some(true),
            use_browser: None,
        });

        let scrape_res = self.tool_executor.execute(scrape_call).await?;

        if let AgentToolResult::Scrape {
            url,
            title,
            markdown,
            text,
            ..
        } = scrape_res
        {
            let effective_title = title.or_else(|| suggested_title.map(|s| s.to_string()));
            let content_text = markdown.or(text).unwrap_or_default();

            if content_text.trim().is_empty() {
                return Ok(());
            }

            let source_type = AgentPolicy::classify_source(&url, effective_title.as_deref());

            // Extract bounded evidence paragraphs/passages
            for paragraph in content_text.split("\n\n") {
                let trimmed = paragraph.trim();
                if trimmed.len() >= 40 && trimmed.len() <= 2000 {
                    let hash = compute_chunk_hash(trimmed);
                    let evidence = Evidence {
                        id: String::new(), // allocated automatically
                        source_url: url.clone(),
                        source_title: effective_title.clone(),
                        source_type,
                        content_hash: hash,
                        chunk_id: None,
                        excerpt: trimmed.to_string(),
                        extracted_fact: None,
                        relevance: 0.85,
                        published_at: None,
                        fetched_at: Utc::now(),
                    };
                    state.evidence.insert(evidence);
                }
            }
        }

        Ok(())
    }

    fn build_error_result(
        &self,
        job_id: String,
        task: String,
        state: Arc<AgentState>,
        err_msg: String,
    ) -> AgentResult {
        let stats = state.tracker.stats(state.evidence.unique_sources_count());
        AgentResult {
            job_id,
            task,
            status: AgentStatus::Failed,
            success: false,
            data: None,
            text_answer: None,
            markdown_answer: None,
            research: stats,
            citations: Vec::new(),
            claims: Vec::new(),
            conflicts: Vec::new(),
            confidence: None,
            warnings: Vec::new(),
            trace: None,
            error: Some(err_msg),
        }
    }

    fn build_cancelled_result(
        &self,
        job_id: String,
        task: String,
        state: Arc<AgentState>,
        warnings: Vec<String>,
    ) -> AgentResult {
        let stats = state.tracker.stats(state.evidence.unique_sources_count());
        AgentResult {
            job_id,
            task,
            status: AgentStatus::Cancelled,
            success: false,
            data: None,
            text_answer: Some("Research job was cancelled before completion.".to_string()),
            markdown_answer: Some(
                "## Job Cancelled\n\nResearch was cancelled before completion.".to_string(),
            ),
            research: stats,
            citations: Vec::new(),
            claims: Vec::new(),
            conflicts: Vec::new(),
            confidence: None,
            warnings,
            trace: None,
            error: Some("Job cancelled by user".to_string()),
        }
    }
}
