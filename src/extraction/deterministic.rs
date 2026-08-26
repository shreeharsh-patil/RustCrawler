use crate::extraction::candidate::CandidatePool;
use crate::extraction::models::{ExtractedField, FieldProvenance, MissingFieldBehavior};
use serde_json::{Map, Value};
use std::collections::HashMap;
use url::Url;

pub struct DeterministicExtractor;

impl DeterministicExtractor {
    /// Attempts to extract structured data matching the schema deterministically from candidates.
    /// Returns (extracted_value, provenance_map, confidence_sufficient).
    pub fn extract(
        pool: &CandidatePool,
        schema: Option<&Value>,
        _prompt: Option<&str>,
        source_url: &Url,
        missing_behavior: MissingFieldBehavior,
    ) -> (Option<Value>, HashMap<String, FieldProvenance>, bool) {
        if let Some(schema_val) = schema {
            Self::extract_with_schema(pool, schema_val, source_url, missing_behavior)
        } else {
            Self::extract_without_schema(pool, source_url)
        }
    }

    fn extract_with_schema(
        pool: &CandidatePool,
        schema: &Value,
        source_url: &Url,
        missing_behavior: MissingFieldBehavior,
    ) -> (Option<Value>, HashMap<String, FieldProvenance>, bool) {
        let schema_obj = match schema.as_object() {
            Some(o) => o,
            None => return (None, HashMap::new(), false),
        };

        let schema_type = schema_obj
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("object");

        if schema_type == "array" {
            // Array extraction: Look for array items in pool or generate from matching tables
            let (arr_val, prov, is_satisfied) =
                Self::extract_array_schema(pool, schema_obj, source_url);
            return (Some(arr_val), prov, is_satisfied);
        }

        // Object extraction
        let properties = match schema_obj.get("properties").and_then(|p| p.as_object()) {
            Some(p) => p,
            None => return (None, HashMap::new(), false),
        };

        let required_fields: Vec<String> = schema_obj
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let mut output_map = Map::new();
        let mut provenance_map = HashMap::new();
        let mut all_required_satisfied = true;
        let mut found_fields_count = 0;

        for (prop_name, prop_schema) in properties {
            let matched = Self::find_candidate_for_property(pool, prop_name, prop_schema);

            if let Some(field) = matched {
                output_map.insert(prop_name.clone(), field.value.clone());
                provenance_map.insert(
                    prop_name.clone(),
                    FieldProvenance {
                        field_path: prop_name.clone(),
                        source_url: source_url.to_string(),
                        chunk_id: None,
                        source: field.source,
                        excerpt: field.excerpt,
                    },
                );
                found_fields_count += 1;
            } else if required_fields.contains(prop_name) {
                all_required_satisfied = false;
                match missing_behavior {
                    MissingFieldBehavior::Null => {
                        output_map.insert(prop_name.clone(), Value::Null);
                    }
                    MissingFieldBehavior::Omit | MissingFieldBehavior::Error => {}
                }
            } else if missing_behavior == MissingFieldBehavior::Null {
                output_map.insert(prop_name.clone(), Value::Null);
            }
        }

        // If no fields at all could be deterministically matched, signal failure
        if found_fields_count == 0 {
            return (None, provenance_map, false);
        }

        let is_confident =
            all_required_satisfied && (found_fields_count >= required_fields.len().max(1));
        (
            Some(Value::Object(output_map)),
            provenance_map,
            is_confident,
        )
    }

    fn extract_array_schema(
        pool: &CandidatePool,
        schema_obj: &Map<String, Value>,
        source_url: &Url,
    ) -> (Value, HashMap<String, FieldProvenance>, bool) {
        let mut array_items = Vec::new();
        let mut provenance_map = HashMap::new();

        let _item_schema = schema_obj.get("items");

        // Check if candidate pool contains array values
        for (key, candidate) in &pool.candidates {
            if let Value::Array(items) = &candidate.value {
                for (idx, item) in items.iter().enumerate() {
                    array_items.push(item.clone());
                    provenance_map.insert(
                        format!("[{idx}]"),
                        FieldProvenance {
                            field_path: format!("{key}[{idx}]"),
                            source_url: source_url.to_string(),
                            chunk_id: None,
                            source: candidate.source,
                            excerpt: candidate.excerpt.clone(),
                        },
                    );
                }
                break;
            }
        }

        let is_satisfied = !array_items.is_empty();
        (Value::Array(array_items), provenance_map, is_satisfied)
    }

    fn extract_without_schema(
        pool: &CandidatePool,
        source_url: &Url,
    ) -> (Option<Value>, HashMap<String, FieldProvenance>, bool) {
        if pool.candidates.is_empty() {
            return (None, HashMap::new(), false);
        }

        let mut output_map = Map::new();
        let mut provenance_map = HashMap::new();

        // Sort candidates by confidence descending
        let mut sorted = pool.candidates.clone();
        sorted.sort_by(|a, b| {
            b.1.confidence
                .partial_cmp(&a.1.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for (k, v) in sorted {
            if !output_map.contains_key(&k) && !k.contains('.') && !k.contains('[') {
                output_map.insert(k.clone(), v.value.clone());
                provenance_map.insert(
                    k.clone(),
                    FieldProvenance {
                        field_path: k,
                        source_url: source_url.to_string(),
                        chunk_id: None,
                        source: v.source,
                        excerpt: v.excerpt,
                    },
                );
            }
        }

        if output_map.is_empty() {
            (None, HashMap::new(), false)
        } else {
            (Some(Value::Object(output_map)), provenance_map, true)
        }
    }

    fn find_candidate_for_property(
        pool: &CandidatePool,
        prop_name: &str,
        prop_schema: &Value,
    ) -> Option<ExtractedField<Value>> {
        let aliases = get_property_aliases(prop_name);
        let expected_type = prop_schema
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("string");

        // First attempt exact key match (highest priority)
        for (k, v) in &pool.candidates {
            if k.eq_ignore_ascii_case(prop_name) {
                if let Some(coerced) = coerce_to_schema_type(&v.value, expected_type) {
                    return Some(ExtractedField {
                        value: coerced,
                        confidence: v.confidence,
                        source: v.source,
                        excerpt: v.excerpt.clone(),
                    });
                }
            }
        }

        // Second attempt known aliases
        for alias in &aliases {
            for (k, v) in &pool.candidates {
                if k.eq_ignore_ascii_case(alias) || k.ends_with(&format!(".{alias}")) {
                    if let Some(coerced) = coerce_to_schema_type(&v.value, expected_type) {
                        return Some(ExtractedField {
                            value: coerced,
                            confidence: v.confidence * 0.90,
                            source: v.source,
                            excerpt: v.excerpt.clone(),
                        });
                    }
                }
            }
        }

        None
    }
}

pub fn get_property_aliases(prop_name: &str) -> Vec<String> {
    let lower = prop_name.to_lowercase();
    let mut aliases = vec![lower.clone()];

    match lower.as_str() {
        "price" | "cost" | "amount" => {
            aliases.extend([
                "product_price".into(),
                "price_amount".into(),
                "offers.price".into(),
                "offers.lowprice".into(),
                "lowprice".into(),
                "price".into(),
            ]);
        }
        "currency" | "price_currency" => {
            aliases.extend([
                "pricecurrency".into(),
                "price_currency".into(),
                "offers.pricecurrency".into(),
                "currency_code".into(),
            ]);
        }
        "name" | "title" | "headline" => {
            aliases.extend([
                "product_name".into(),
                "item_name".into(),
                "og:title".into(),
                "twitter:title".into(),
                "title".into(),
                "name".into(),
            ]);
        }
        "description" | "summary" | "about" => {
            aliases.extend([
                "og:description".into(),
                "twitter:description".into(),
                "summary".into(),
                "details".into(),
                "overview".into(),
            ]);
        }
        "image" | "image_url" | "thumbnail" => {
            aliases.extend([
                "og:image".into(),
                "twitter:image".into(),
                "photo".into(),
                "picture".into(),
                "logo".into(),
            ]);
        }
        "company" | "brand" | "organization" | "publisher" => {
            aliases.extend([
                "brand.name".into(),
                "brand".into(),
                "publisher.name".into(),
                "organization".into(),
                "vendor".into(),
                "site_name".into(),
            ]);
        }
        "availability" | "stock" | "in_stock" => {
            aliases.extend([
                "offers.availability".into(),
                "stock_status".into(),
                "in_stock".into(),
            ]);
        }
        "author" | "creator" => {
            aliases.extend([
                "author.name".into(),
                "creator".into(),
                "by".into(),
                "writer".into(),
            ]);
        }
        "url" | "link" | "website" => {
            aliases.extend([
                "og:url".into(),
                "canonical_url".into(),
                "page_url".into(),
                "link".into(),
            ]);
        }
        "sku" | "mpn" | "product_id" => {
            aliases.extend([
                "identifier".into(),
                "product_id".into(),
                "code".into(),
                "item_id".into(),
            ]);
        }
        _ => {}
    }

    aliases
}

pub fn coerce_to_schema_type(val: &Value, target_type: &str) -> Option<Value> {
    match target_type {
        "string" => match val {
            Value::String(s) => Some(Value::String(s.clone())),
            Value::Number(n) => Some(Value::String(n.to_string())),
            Value::Bool(b) => Some(Value::String(b.to_string())),
            _ => None,
        },
        "number" => match val {
            Value::Number(n) => Some(Value::Number(n.clone())),
            Value::String(s) => {
                // Strip currency symbols and whitespace like "$99.99" -> 99.99
                let clean = s.trim_matches(|c: char| !c.is_numeric() && c != '.' && c != '-');
                if let Ok(flt) = clean.parse::<f64>() {
                    serde_json::Number::from_f64(flt).map(Value::Number)
                } else {
                    None
                }
            }
            _ => None,
        },
        "integer" => match val {
            Value::Number(n) if n.is_i64() || n.is_u64() => Some(Value::Number(n.clone())),
            Value::Number(n) if n.is_f64() => {
                let flt = n.as_f64().unwrap();
                Some(Value::Number((flt.round() as i64).into()))
            }
            Value::String(s) => {
                let clean = s.trim_matches(|c: char| !c.is_numeric() && c != '-');
                clean.parse::<i64>().ok().map(|i| Value::Number(i.into()))
            }
            _ => None,
        },
        "boolean" => match val {
            Value::Bool(b) => Some(Value::Bool(*b)),
            Value::String(s) => match s.trim().to_lowercase().as_str() {
                "true" | "yes" | "1" | "in_stock" | "instock" => Some(Value::Bool(true)),
                "false" | "no" | "0" | "out_of_stock" | "outofstock" => Some(Value::Bool(false)),
                _ => None,
            },
            _ => None,
        },
        "array" => match val {
            Value::Array(arr) => Some(Value::Array(arr.clone())),
            other => Some(Value::Array(vec![other.clone()])),
        },
        "object" => match val {
            Value::Object(map) => Some(Value::Object(map.clone())),
            _ => None,
        },
        _ => Some(val.clone()),
    }
}
