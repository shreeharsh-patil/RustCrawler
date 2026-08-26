use crate::extraction::models::{ContentChunk, ExtractionSource, FieldProvenance};
use serde_json::Value;
use std::collections::HashMap;

pub struct ProvenanceTracker;

impl ProvenanceTracker {
    /// Builds provenance citations for an extracted JSON value using source chunks.
    pub fn build_provenance(
        data: &Value,
        chunks: &[ContentChunk],
        existing_provenance: Option<HashMap<String, FieldProvenance>>,
    ) -> HashMap<String, FieldProvenance> {
        let mut map = existing_provenance.unwrap_or_default();
        Self::trace_provenance(data, "$", chunks, &mut map);
        map
    }

    fn trace_provenance(
        value: &Value,
        path: &str,
        chunks: &[ContentChunk],
        provenance_map: &mut HashMap<String, FieldProvenance>,
    ) {
        if provenance_map.contains_key(path) {
            return;
        }

        match value {
            Value::Object(obj) => {
                for (k, v) in obj {
                    let child_path = if path == "$" {
                        k.clone()
                    } else {
                        format!("{path}.{k}")
                    };
                    Self::trace_provenance(v, &child_path, chunks, provenance_map);
                }
            }
            Value::Array(arr) => {
                for (idx, item) in arr.iter().enumerate() {
                    let child_path = format!("{path}[{idx}]");
                    Self::trace_provenance(item, &child_path, chunks, provenance_map);
                }
            }
            Value::String(s) if !s.trim().is_empty() => {
                let needle = s.trim();
                for chunk in chunks {
                    if let Some(pos) = chunk.content.find(needle) {
                        let start = pos.saturating_sub(40);
                        let end = (pos + needle.len() + 40).min(chunk.content.len());
                        let excerpt = chunk.content[start..end].trim().to_string();

                        provenance_map.insert(
                            path.to_string(),
                            FieldProvenance {
                                field_path: path.to_string(),
                                source_url: chunk.source_url.to_string(),
                                chunk_id: Some(chunk.id.clone()),
                                source: match chunk.content_type {
                                    crate::extraction::models::ChunkType::Json => {
                                        ExtractionSource::EmbeddedJson
                                    }
                                    crate::extraction::models::ChunkType::Table => {
                                        ExtractionSource::HtmlTable
                                    }
                                    crate::extraction::models::ChunkType::Metadata => {
                                        ExtractionSource::MetaTag
                                    }
                                    crate::extraction::models::ChunkType::PdfPage => {
                                        ExtractionSource::PdfText
                                    }
                                    crate::extraction::models::ChunkType::DocxSection => {
                                        ExtractionSource::DocxText
                                    }
                                    _ => ExtractionSource::Llm,
                                },
                                excerpt: Some(excerpt),
                            },
                        );
                        break;
                    }
                }
            }
            Value::Number(n) => {
                let needle = n.to_string();
                for chunk in chunks {
                    if let Some(pos) = chunk.content.find(&needle) {
                        let start = pos.saturating_sub(40);
                        let end = (pos + needle.len() + 40).min(chunk.content.len());
                        let excerpt = chunk.content[start..end].trim().to_string();

                        provenance_map.insert(
                            path.to_string(),
                            FieldProvenance {
                                field_path: path.to_string(),
                                source_url: chunk.source_url.to_string(),
                                chunk_id: Some(chunk.id.clone()),
                                source: ExtractionSource::Llm,
                                excerpt: Some(excerpt),
                            },
                        );
                        break;
                    }
                }
            }
            _ => {}
        }
    }
}
