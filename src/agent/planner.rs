use std::sync::Arc;

use serde_json::Value;

use crate::agent::models::{AgentMode, AgentRequest, ResearchPlan, ResearchSubgoal, SubgoalStatus};
use crate::config::Config;
use crate::error::CrawlerError;
use crate::extraction::llm::LlmProvider;
use crate::extraction::models::LlmExtractionRequest;

/// Autonomous planner generating bounded research subgoals deterministically or via LLM reasoning
pub struct AgentPlanner {
    config: Arc<Config>,
    llm: Option<Arc<dyn LlmProvider>>,
}

impl AgentPlanner {
    pub fn new(config: Arc<Config>, llm: Option<Arc<dyn LlmProvider>>) -> Self {
        Self { config, llm }
    }

    /// Creates a bounded research plan for the incoming request
    pub async fn create_plan(&self, req: &AgentRequest) -> Result<ResearchPlan, CrawlerError> {
        // 1. If explicit URLs are provided or deterministic mode matches, build deterministic plan
        if !req.initial_urls.is_empty() {
            return Ok(self.build_direct_urls_plan(req));
        }

        match req.mode {
            AgentMode::Comparison => Ok(self.build_comparison_plan(req)),
            AgentMode::SiteAnalysis => Ok(self.build_site_analysis_plan(req)),
            AgentMode::StructuredResearch => Ok(self.build_structured_research_plan(req)),
            AgentMode::MonitoringAnalysis => Ok(self.build_monitoring_plan(req)),
            AgentMode::Research => {
                // If LLM is configured and enabled, attempt dynamic planning
                if self.config.llm_enabled && self.llm.is_some() {
                    if let Ok(plan) = self.plan_with_llm(req).await {
                        return Ok(plan);
                    }
                }
                // Fallback to deterministic research plan
                Ok(self.build_general_research_plan(req))
            }
        }
    }

    fn build_direct_urls_plan(&self, req: &AgentRequest) -> ResearchPlan {
        let mut subgoals = Vec::new();

        for (idx, url) in req.initial_urls.iter().enumerate() {
            subgoals.push(ResearchSubgoal {
                id: format!("subgoal_{}", idx + 1),
                description: format!("Scrape and analyze content from direct URL: {}", url),
                status: SubgoalStatus::Pending,
                priority: 1,
                target_urls: vec![url.clone()],
                search_queries: Vec::new(),
                extracted_facts: Vec::new(),
            });
        }

        subgoals.push(ResearchSubgoal {
            id: format!("subgoal_{}", subgoals.len() + 1),
            description: "Extract structured facts and evaluate evidence completeness".to_string(),
            status: SubgoalStatus::Pending,
            priority: 2,
            target_urls: Vec::new(),
            search_queries: Vec::new(),
            extracted_facts: Vec::new(),
        });

        subgoals.push(ResearchSubgoal {
            id: format!("subgoal_{}", subgoals.len() + 1),
            description: "Synthesize findings and generate citation-grounded response".to_string(),
            status: SubgoalStatus::Pending,
            priority: 3,
            target_urls: Vec::new(),
            search_queries: Vec::new(),
            extracted_facts: Vec::new(),
        });

        ResearchPlan {
            objective: req.task.clone(),
            subgoals,
            expected_output: req.output_schema.clone(),
        }
    }

    fn build_comparison_plan(&self, req: &AgentRequest) -> ResearchPlan {
        let mut subgoals = Vec::new();
        let entities = extract_entities_from_task(&req.task);

        if entities.len() >= 2 {
            for (i, entity) in entities.iter().enumerate() {
                subgoals.push(ResearchSubgoal {
                    id: format!("subgoal_{}", i + 1),
                    description: format!(
                        "Research and gather primary source evidence for {}",
                        entity
                    ),
                    status: SubgoalStatus::Pending,
                    priority: 1,
                    target_urls: Vec::new(),
                    search_queries: vec![
                        format!("{} official documentation features", entity),
                        format!("{} architecture performance", entity),
                    ],
                    extracted_facts: Vec::new(),
                });
            }

            subgoals.push(ResearchSubgoal {
                id: format!("subgoal_{}", entities.len() + 1),
                description:
                    "Synthesize side-by-side comparison matrix and identify conflicting claims"
                        .to_string(),
                status: SubgoalStatus::Pending,
                priority: 2,
                target_urls: Vec::new(),
                search_queries: vec![req.task.clone()],
                extracted_facts: Vec::new(),
            });
        } else {
            subgoals.push(ResearchSubgoal {
                id: "subgoal_1".to_string(),
                description:
                    "Search and identify authoritative primary sources for comparison targets"
                        .to_string(),
                status: SubgoalStatus::Pending,
                priority: 1,
                target_urls: req.initial_urls.clone(),
                search_queries: vec![req.task.clone()],
                extracted_facts: Vec::new(),
            });

            subgoals.push(ResearchSubgoal {
                id: "subgoal_2".to_string(),
                description: "Scrape official repositories, documentation and benchmarks"
                    .to_string(),
                status: SubgoalStatus::Pending,
                priority: 2,
                target_urls: Vec::new(),
                search_queries: Vec::new(),
                extracted_facts: Vec::new(),
            });

            subgoals.push(ResearchSubgoal {
                id: "subgoal_3".to_string(),
                description: "Evaluate evidence gaps, detect contradictions, and synthesize comparison matrix".to_string(),
                status: SubgoalStatus::Pending,
                priority: 3,
                target_urls: Vec::new(),
                search_queries: Vec::new(),
                extracted_facts: Vec::new(),
            });
        }

        ResearchPlan {
            objective: req.task.clone(),
            subgoals,
            expected_output: req.output_schema.clone(),
        }
    }

    fn build_site_analysis_plan(&self, req: &AgentRequest) -> ResearchPlan {
        let subgoals = vec![
            ResearchSubgoal {
                id: "subgoal_1".to_string(),
                description: "Map website structure and discover top-level navigational endpoints"
                    .to_string(),
                status: SubgoalStatus::Pending,
                priority: 1,
                target_urls: req.initial_urls.clone(),
                search_queries: vec![req.task.clone()],
                extracted_facts: Vec::new(),
            },
            ResearchSubgoal {
                id: "subgoal_2".to_string(),
                description: "Scrape landing page, pricing, docs, and core informational pages"
                    .to_string(),
                status: SubgoalStatus::Pending,
                priority: 2,
                target_urls: Vec::new(),
                search_queries: Vec::new(),
                extracted_facts: Vec::new(),
            },
            ResearchSubgoal {
                id: "subgoal_3".to_string(),
                description: "Extract architecture, offerings, tech stack, and metadata"
                    .to_string(),
                status: SubgoalStatus::Pending,
                priority: 3,
                target_urls: Vec::new(),
                search_queries: Vec::new(),
                extracted_facts: Vec::new(),
            },
            ResearchSubgoal {
                id: "subgoal_4".to_string(),
                description: "Synthesize structured site analysis audit report".to_string(),
                status: SubgoalStatus::Pending,
                priority: 4,
                target_urls: Vec::new(),
                search_queries: Vec::new(),
                extracted_facts: Vec::new(),
            },
        ];

        ResearchPlan {
            objective: req.task.clone(),
            subgoals,
            expected_output: req.output_schema.clone(),
        }
    }

    fn build_structured_research_plan(&self, req: &AgentRequest) -> ResearchPlan {
        let subgoals = vec![
            ResearchSubgoal {
                id: "subgoal_1".to_string(),
                description: "Generate targeted discovery queries matching required schema fields".to_string(),
                status: SubgoalStatus::Pending,
                priority: 1,
                target_urls: req.initial_urls.clone(),
                search_queries: vec![req.task.clone()],
                extracted_facts: Vec::new(),
            },
            ResearchSubgoal {
                id: "subgoal_2".to_string(),
                description: "Scrape candidate primary sources and extract structured records".to_string(),
                status: SubgoalStatus::Pending,
                priority: 2,
                target_urls: Vec::new(),
                search_queries: Vec::new(),
                extracted_facts: Vec::new(),
            },
            ResearchSubgoal {
                id: "subgoal_3".to_string(),
                description: "Verify schema constraints, identify missing fields, and run targeted gap-filling".to_string(),
                status: SubgoalStatus::Pending,
                priority: 3,
                target_urls: Vec::new(),
                search_queries: Vec::new(),
                extracted_facts: Vec::new(),
            },
            ResearchSubgoal {
                id: "subgoal_4".to_string(),
                description: "Assemble verified structured data with complete field-level citations".to_string(),
                status: SubgoalStatus::Pending,
                priority: 4,
                target_urls: Vec::new(),
                search_queries: Vec::new(),
                extracted_facts: Vec::new(),
            },
        ];

        ResearchPlan {
            objective: req.task.clone(),
            subgoals,
            expected_output: req.output_schema.clone(),
        }
    }

    fn build_monitoring_plan(&self, req: &AgentRequest) -> ResearchPlan {
        let subgoals = vec![
            ResearchSubgoal {
                id: "subgoal_1".to_string(),
                description: "Check cached crawl baselines and index for changes".to_string(),
                status: SubgoalStatus::Pending,
                priority: 1,
                target_urls: req.initial_urls.clone(),
                search_queries: vec![req.task.clone()],
                extracted_facts: Vec::new(),
            },
            ResearchSubgoal {
                id: "subgoal_2".to_string(),
                description: "Scrape latest pages and compute differential checksums".to_string(),
                status: SubgoalStatus::Pending,
                priority: 2,
                target_urls: Vec::new(),
                search_queries: Vec::new(),
                extracted_facts: Vec::new(),
            },
            ResearchSubgoal {
                id: "subgoal_3".to_string(),
                description: "Synthesize change summary and alert anomalies".to_string(),
                status: SubgoalStatus::Pending,
                priority: 3,
                target_urls: Vec::new(),
                search_queries: Vec::new(),
                extracted_facts: Vec::new(),
            },
        ];

        ResearchPlan {
            objective: req.task.clone(),
            subgoals,
            expected_output: req.output_schema.clone(),
        }
    }

    fn build_general_research_plan(&self, req: &AgentRequest) -> ResearchPlan {
        let search_queries = vec![
            req.task.clone(),
            format!("{} overview documentation", req.task),
            format!("{} details facts", req.task),
        ];

        let subgoals = vec![
            ResearchSubgoal {
                id: "subgoal_1".to_string(),
                description: "Search and select high-authority primary sources".to_string(),
                status: SubgoalStatus::Pending,
                priority: 1,
                target_urls: Vec::new(),
                search_queries,
                extracted_facts: Vec::new(),
            },
            ResearchSubgoal {
                id: "subgoal_2".to_string(),
                description: "Scrape selected pages and extract relevant evidence passages"
                    .to_string(),
                status: SubgoalStatus::Pending,
                priority: 2,
                target_urls: Vec::new(),
                search_queries: Vec::new(),
                extracted_facts: Vec::new(),
            },
            ResearchSubgoal {
                id: "subgoal_3".to_string(),
                description: "Evaluate evidence coverage and fill remaining gaps".to_string(),
                status: SubgoalStatus::Pending,
                priority: 3,
                target_urls: Vec::new(),
                search_queries: Vec::new(),
                extracted_facts: Vec::new(),
            },
            ResearchSubgoal {
                id: "subgoal_4".to_string(),
                description: "Synthesize comprehensive research response with validated citations"
                    .to_string(),
                status: SubgoalStatus::Pending,
                priority: 4,
                target_urls: Vec::new(),
                search_queries: Vec::new(),
                extracted_facts: Vec::new(),
            },
        ];

        ResearchPlan {
            objective: req.task.clone(),
            subgoals,
            expected_output: req.output_schema.clone(),
        }
    }

    async fn plan_with_llm(&self, req: &AgentRequest) -> Result<ResearchPlan, CrawlerError> {
        let llm = self.llm.as_ref().ok_or(CrawlerError::LlmDisabled)?;

        let prompt = format!(
            "You are a bounded research planner for an autonomous web crawler.\n\
             Task: {}\n\
             Output a JSON object with this exact structure:\n\
             {{\n\
               \"objective\": \"...\",\n\
               \"subgoals\": [\n\
                 {{\n\
                   \"id\": \"subgoal_1\",\n\
                   \"description\": \"...\",\n\
                   \"priority\": 1,\n\
                   \"search_queries\": [\"query 1\", \"query 2\"]\n\
                 }}\n\
               ]\n\
             }}\n\
             Generate 3 to 5 bounded subgoals with focused search queries. Do not include executable code.",
            req.task
        );

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "objective": { "type": "string" },
                "subgoals": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "description": { "type": "string" },
                            "priority": { "type": "integer" },
                            "search_queries": { "type": "array", "items": { "type": "string" } }
                        },
                        "required": ["id", "description"]
                    }
                }
            },
            "required": ["objective", "subgoals"]
        });

        let llm_req = LlmExtractionRequest {
            system_prompt: "You are a research planning assistant. Produce clean, strictly-validated JSON plans without commentary.".to_string(),
            user_prompt: prompt,
            schema: Some(schema),
            model: self.config.agent_planner_model.clone(),
            temperature: Some(0.1),
            max_tokens: Some(1000),
        };

        let resp = llm.structured_generate(llm_req).await?;
        let parsed: Value = if let Some(parsed) = resp.parsed_json {
            parsed
        } else {
            serde_json::from_str(&resp.raw_content).map_err(|e| {
                CrawlerError::AgentPlanFailed(format!("Failed to parse LLM plan JSON: {e}"))
            })?
        };

        let objective = parsed["objective"]
            .as_str()
            .unwrap_or(&req.task)
            .to_string();

        let mut subgoals = Vec::new();
        if let Some(arr) = parsed["subgoals"].as_array() {
            for (i, val) in arr.iter().enumerate() {
                let id = val["id"]
                    .as_str()
                    .unwrap_or(&format!("subgoal_{}", i + 1))
                    .to_string();
                let description = val["description"]
                    .as_str()
                    .unwrap_or("Gather research evidence")
                    .to_string();
                let priority = val["priority"].as_u64().unwrap_or(1) as u8;
                let queries = val["search_queries"]
                    .as_array()
                    .map(|q_arr| {
                        q_arr
                            .iter()
                            .filter_map(|q| q.as_str().map(|s| s.to_string()))
                            .take(self.config.agent_max_query_variants_per_subgoal as usize)
                            .collect()
                    })
                    .unwrap_or_default();

                subgoals.push(ResearchSubgoal {
                    id,
                    description,
                    status: SubgoalStatus::Pending,
                    priority,
                    target_urls: Vec::new(),
                    search_queries: queries,
                    extracted_facts: Vec::new(),
                });
            }
        }

        if subgoals.is_empty() {
            return Err(CrawlerError::AgentPlanFailed(
                "LLM produced 0 subgoals".to_string(),
            ));
        }

        Ok(ResearchPlan {
            objective,
            subgoals,
            expected_output: req.output_schema.clone(),
        })
    }
}

fn extract_entities_from_task(task: &str) -> Vec<String> {
    let lower = task.to_lowercase();
    let mut entities = Vec::new();

    let cleaned = if let Some(idx) = lower.find("compare") {
        &task[idx + 7..]
    } else if let Some(idx) = lower.find("comparison of") {
        &task[idx + 13..]
    } else if let Some(idx) = lower.find("between") {
        &task[idx + 7..]
    } else {
        task
    };

    let normalized = cleaned
        .replace(" vs. ", " | ")
        .replace(" vs ", " | ")
        .replace(" versus ", " | ")
        .replace(" against ", " | ")
        .replace(" and ", " | ")
        .replace(" with ", " | ")
        .replace([',', '&'], " | ");

    for part in normalized.split('|') {
        let words: Vec<&str> = part.split_whitespace().collect();
        if !words.is_empty() {
            let entity = words[0].trim_matches(|c: char| !c.is_alphanumeric());
            let entity_lower = entity.to_lowercase();
            if !entity.is_empty()
                && entity.len() > 1
                && !matches!(
                    entity_lower.as_str(),
                    "the"
                        | "their"
                        | "and"
                        | "top"
                        | "best"
                        | "performance"
                        | "features"
                        | "model"
                        | "pricing"
                )
            {
                entities.push(entity.to_string());
            }
        }
    }

    entities
}
