use crate::knowledge::lexical::LexicalMatch;
use crate::knowledge::models::HybridMatch;
use crate::knowledge::vector::VectorMatch;
use std::collections::HashMap;

/// Performs Reciprocal Rank Fusion (RRF) combining vector and lexical rankings
pub fn reciprocal_rank_fusion(
    vector_matches: &[VectorMatch],
    lexical_matches: &[LexicalMatch],
    k: f64, // Standard k=60.0
    max_chunks_per_document: usize,
    limit: usize,
) -> Vec<HybridMatch> {
    let mut fusion_map: HashMap<String, HybridMatchBuilder> = HashMap::new();

    // 1. Process vector rankings (rank starts at 1)
    for (rank_idx, v_match) in vector_matches.iter().enumerate() {
        let rank = rank_idx + 1;
        let rrf_score = 1.0 / (k + rank as f64);

        let entry = fusion_map
            .entry(v_match.chunk_id.clone())
            .or_insert_with(|| HybridMatchBuilder {
                chunk_id: v_match.chunk_id.clone(),
                document_id: v_match.document_id.clone(),
                source_url: v_match.source_url.clone(),
                title: v_match.title.clone(),
                heading_path: v_match.heading_path.clone(),
                text: v_match.text.clone(),
                vector_score: Some(v_match.score),
                lexical_score: None,
                vector_rank: Some(rank),
                lexical_rank: None,
                rrf_score: 0.0,
                metadata: v_match.metadata.clone(),
                document_metadata: v_match.document_metadata.clone(),
            });

        entry.vector_rank = Some(rank);
        entry.vector_score = Some(v_match.score);
        entry.rrf_score += rrf_score;
    }

    // 2. Process lexical rankings (rank starts at 1)
    for (rank_idx, l_match) in lexical_matches.iter().enumerate() {
        let rank = rank_idx + 1;
        let rrf_score = 1.0 / (k + rank as f64);

        let entry = fusion_map
            .entry(l_match.chunk_id.clone())
            .or_insert_with(|| HybridMatchBuilder {
                chunk_id: l_match.chunk_id.clone(),
                document_id: l_match.document_id.clone(),
                source_url: l_match.source_url.clone(),
                title: l_match.title.clone(),
                heading_path: l_match.heading_path.clone(),
                text: l_match.text.clone(),
                vector_score: None,
                lexical_score: Some(l_match.score),
                vector_rank: None,
                lexical_rank: Some(rank),
                rrf_score: 0.0,
                metadata: l_match.metadata.clone(),
                document_metadata: l_match.document_metadata.clone(),
            });

        entry.lexical_rank = Some(rank);
        entry.lexical_score = Some(l_match.score);
        entry.rrf_score += rrf_score;
    }

    // 3. Convert to list and sort descending by total RRF score
    let mut all_matches: Vec<HybridMatch> = fusion_map
        .into_values()
        .map(|builder| HybridMatch {
            chunk_id: builder.chunk_id,
            document_id: builder.document_id,
            source_url: builder.source_url,
            title: builder.title,
            heading_path: builder.heading_path,
            text: builder.text,
            score: builder.rrf_score,
            lexical_rank: builder.lexical_rank,
            vector_rank: builder.vector_rank,
            rrf_score: builder.rrf_score,
            metadata: builder.metadata,
            document_metadata: builder.document_metadata,
        })
        .collect();

    all_matches.sort_by(|a, b| {
        b.rrf_score
            .partial_cmp(&a.rrf_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // 4. Apply search diversification (max_chunks_per_document)
    if max_chunks_per_document > 0 {
        let mut diversified = Vec::new();
        let mut doc_chunk_counts: HashMap<String, usize> = HashMap::new();

        for m in all_matches {
            let count = doc_chunk_counts.entry(m.document_id.clone()).or_insert(0);
            if *count < max_chunks_per_document {
                *count += 1;
                diversified.push(m);
                if diversified.len() >= limit {
                    break;
                }
            }
        }
        diversified
    } else {
        if all_matches.len() > limit {
            all_matches.truncate(limit);
        }
        all_matches
    }
}

struct HybridMatchBuilder {
    chunk_id: String,
    document_id: String,
    source_url: Option<String>,
    title: Option<String>,
    heading_path: Vec<String>,
    text: String,
    #[allow(dead_code)]
    vector_score: Option<f64>,
    #[allow(dead_code)]
    lexical_score: Option<f64>,
    vector_rank: Option<usize>,
    lexical_rank: Option<usize>,
    rrf_score: f64,
    metadata: crate::knowledge::models::ChunkMetadata,
    document_metadata: Option<crate::knowledge::models::IndexMetadata>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::models::ChunkMetadata;

    #[test]
    fn test_reciprocal_rank_fusion_combined_ranking() {
        let v_match1 = VectorMatch {
            chunk_id: "chk_1".to_string(),
            document_id: "doc_1".to_string(),
            source_url: None,
            title: None,
            heading_path: Vec::new(),
            text: "Chunk 1".to_string(),
            score: 0.95,
            metadata: ChunkMetadata::default(),
            document_metadata: None,
        };
        let v_match2 = VectorMatch {
            chunk_id: "chk_2".to_string(),
            document_id: "doc_2".to_string(),
            source_url: None,
            title: None,
            heading_path: Vec::new(),
            text: "Chunk 2".to_string(),
            score: 0.85,
            metadata: ChunkMetadata::default(),
            document_metadata: None,
        };

        let l_match2 = LexicalMatch {
            chunk_id: "chk_2".to_string(),
            document_id: "doc_2".to_string(),
            source_url: None,
            title: None,
            heading_path: Vec::new(),
            text: "Chunk 2".to_string(),
            score: 12.0,
            metadata: ChunkMetadata::default(),
            document_metadata: None,
        };
        let l_match3 = LexicalMatch {
            chunk_id: "chk_3".to_string(),
            document_id: "doc_3".to_string(),
            source_url: None,
            title: None,
            heading_path: Vec::new(),
            text: "Chunk 3".to_string(),
            score: 8.0,
            metadata: ChunkMetadata::default(),
            document_metadata: None,
        };

        // chk_2 is rank 2 in vector and rank 1 in lexical -> should win RRF fusion!
        let fused =
            reciprocal_rank_fusion(&[v_match1, v_match2], &[l_match2, l_match3], 60.0, 5, 10);

        assert_eq!(fused.len(), 3);
        assert_eq!(
            fused[0].chunk_id, "chk_2",
            "chk_2 should rank first due to appearing in both rankings"
        );
        assert_eq!(fused[0].vector_rank, Some(2));
        assert_eq!(fused[0].lexical_rank, Some(1));
    }
}
