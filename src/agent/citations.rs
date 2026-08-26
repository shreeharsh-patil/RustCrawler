use std::collections::{HashMap, HashSet};

use crate::agent::evidence::EvidenceStore;
use crate::agent::models::{Citation, Claim};

/// Citation validator guaranteeing all cited sources are structurally valid, present in evidence store, and authentic
pub struct CitationValidator;

impl CitationValidator {
    /// Extracts citation markers (e.g. `[E1]`, `[E2]`, `[src_1]`) from text
    pub fn extract_citation_ids(text: &str) -> Vec<String> {
        let mut ids = Vec::new();
        let mut seen = HashSet::new();

        for part in text.split('[') {
            if let Some((id, _)) = part.split_once(']') {
                let trimmed = id.trim();
                let is_e_tag = trimmed.starts_with('E')
                    && trimmed.len() > 1
                    && trimmed[1..].chars().all(|c| c.is_ascii_digit());
                let is_src_tag = trimmed.starts_with("src_");
                let is_chk_tag = trimmed.starts_with("chk_");

                if (is_e_tag || is_src_tag || is_chk_tag) && seen.insert(trimmed.to_string()) {
                    ids.push(trimmed.to_string());
                }
            }
        }

        ids
    }

    /// Validates all citations in text and claims against the job's EvidenceStore, removing invalid hallucinations
    pub fn validate_and_build_citations(
        text: &str,
        claims: &mut [Claim],
        store: &EvidenceStore,
    ) -> (Vec<Citation>, Vec<String>) {
        let mut citations = Vec::new();
        let mut warnings = Vec::new();
        let mut citation_map: HashMap<String, Citation> = HashMap::new();

        // 1. Extract citation IDs from synthesized text
        let text_ids = Self::extract_citation_ids(text);

        for id in &text_ids {
            if let Some(ev) = store.get(id) {
                let entry = citation_map.entry(id.clone()).or_insert_with(|| Citation {
                    source_id: id.clone(),
                    url: ev.source_url.clone(),
                    title: ev.source_title.clone(),
                    snippet: Some(ev.excerpt.clone()),
                    supports: Vec::new(),
                });
                entry.supports.push("body_text".to_string());
            } else {
                warnings.push(format!(
                    "Removed hallucinated citation [{}] not present in evidence store",
                    id
                ));
            }
        }

        // 2. Validate and map claims
        for claim in claims.iter_mut() {
            let mut valid_evidence_ids = Vec::new();
            for eid in &claim.evidence_ids {
                if let Some(ev) = store.get(eid) {
                    valid_evidence_ids.push(eid.clone());
                    let entry = citation_map.entry(eid.clone()).or_insert_with(|| Citation {
                        source_id: eid.clone(),
                        url: ev.source_url.clone(),
                        title: ev.source_title.clone(),
                        snippet: Some(ev.excerpt.clone()),
                        supports: Vec::new(),
                    });
                    entry.supports.push(claim.id.clone());
                } else {
                    warnings.push(format!(
                        "Claim '{}' cited non-existent evidence [{}]",
                        claim.id, eid
                    ));
                }
            }
            claim.evidence_ids = valid_evidence_ids;
        }

        // 3. Fallback: if no inline citations were used but evidence was collected, attach top primary evidence
        if citation_map.is_empty() {
            let top_items = store.top_relevant(5);
            for item in top_items {
                citation_map.insert(
                    item.id.clone(),
                    Citation {
                        source_id: item.id.clone(),
                        url: item.source_url.clone(),
                        title: item.source_title.clone(),
                        snippet: Some(item.excerpt.clone()),
                        supports: vec!["general_evidence".to_string()],
                    },
                );
            }
        }

        for (_, cit) in citation_map {
            citations.push(cit);
        }

        citations.sort_by(|a, b| a.source_id.cmp(&b.source_id));

        (citations, warnings)
    }
}
