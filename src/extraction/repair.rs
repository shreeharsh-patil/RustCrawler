use crate::error::CrawlerError;
use crate::extraction::llm::LlmProvider;
use crate::extraction::models::{LlmExtractionRequest, MissingFieldBehavior};
use crate::extraction::validation::JsonValidator;
use serde_json::{Map, Value};
use std::sync::Arc;
use tracing::debug;

pub struct ExtractionRepairPipeline;

impl ExtractionRepairPipeline {
    /// Attempts deterministic coercion first, then an optional single LLM repair attempt.
    pub async fn repair_and_validate(
        raw_value: Value,
        schema: &Value,
        missing_behavior: MissingFieldBehavior,
        provider: Option<Arc<dyn LlmProvider>>,
        max_repair_attempts: usize,
    ) -> Result<(Value, bool), CrawlerError> {
        // Step 1: Safe deterministic coercion
        let mut coerced = Self::deterministic_coerce(&raw_value, schema, missing_behavior);

        // Step 2: Validate
        if let Ok(()) = JsonValidator::validate(&coerced, schema) {
            return Ok((coerced, false));
        }

        // Step 3: If validation failed and LLM repair is permitted
        if max_repair_attempts > 0 {
            if let Some(ref llm) = provider {
                let initial_errors = JsonValidator::validate(&coerced, schema)
                    .err()
                    .unwrap_or_default();
                debug!(
                    "Attempting LLM repair for extraction validation errors: {:?}",
                    initial_errors
                );

                let repair_system = "You are a JSON repair specialist.\n\
                You are given a target JSON schema, an invalid JSON response, and validation error messages.\n\
                Fix the JSON so it strictly adheres to the schema and resolves all validation errors.\n\
                Output ONLY the repaired JSON with no markdown formatting or commentary.";

                let repair_user = format!(
                    "TARGET SCHEMA:\n{}\n\nINVALID JSON:\n{}\n\nVALIDATION ERRORS:\n{}\n\nRepaired JSON:",
                    serde_json::to_string_pretty(schema).unwrap_or_default(),
                    serde_json::to_string_pretty(&coerced).unwrap_or_default(),
                    initial_errors.join("\n")
                );

                let req = LlmExtractionRequest {
                    system_prompt: repair_system.to_string(),
                    user_prompt: repair_user,
                    schema: Some(schema.clone()),
                    model: None,
                    temperature: Some(0.0),
                    max_tokens: None,
                };

                if let Ok(resp) = llm.structured_generate(req).await {
                    if let Some(repaired_val) = resp.parsed_json {
                        coerced =
                            Self::deterministic_coerce(&repaired_val, schema, missing_behavior);
                        if let Ok(()) = JsonValidator::validate(&coerced, schema) {
                            return Ok((coerced, true)); // repaired via LLM
                        }
                    }
                }
            }
        }

        // Final check
        match JsonValidator::validate(&coerced, schema) {
            Ok(()) => Ok((coerced, false)),
            Err(errors) => Err(CrawlerError::SchemaValidationFailed(errors.join("; "))),
        }
    }

    /// Recursively coerces types (e.g. numeric strings -> numbers, boolean strings -> booleans).
    pub fn deterministic_coerce(
        value: &Value,
        schema: &Value,
        missing_behavior: MissingFieldBehavior,
    ) -> Value {
        let schema_obj = match schema.as_object() {
            Some(o) => o,
            None => return value.clone(),
        };

        let target_type = schema_obj
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("object");

        match target_type {
            "object" => {
                let mut map = match value.as_object() {
                    Some(m) => m.clone(),
                    None => Map::new(),
                };

                if let Some(props) = schema_obj.get("properties").and_then(|p| p.as_object()) {
                    for (prop_name, prop_schema) in props {
                        if let Some(child_val) = map.get(prop_name) {
                            let coerced_child = Self::deterministic_coerce(
                                child_val,
                                prop_schema,
                                missing_behavior,
                            );
                            map.insert(prop_name.clone(), coerced_child);
                        } else if missing_behavior == MissingFieldBehavior::Null {
                            map.insert(prop_name.clone(), Value::Null);
                        }
                    }
                }

                Value::Object(map)
            }
            "array" => {
                let items_schema = schema_obj.get("items");
                let arr = match value {
                    Value::Array(a) => a.clone(),
                    other => vec![other.clone()],
                };

                if let Some(item_s) = items_schema {
                    Value::Array(
                        arr.iter()
                            .map(|item| Self::deterministic_coerce(item, item_s, missing_behavior))
                            .collect(),
                    )
                } else {
                    Value::Array(arr)
                }
            }
            "number" => match value {
                Value::Number(_) => value.clone(),
                Value::String(s) => {
                    let clean = s.trim_matches(|c: char| !c.is_numeric() && c != '.' && c != '-');
                    if let Ok(flt) = clean.parse::<f64>() {
                        serde_json::Number::from_f64(flt)
                            .map(Value::Number)
                            .unwrap_or(value.clone())
                    } else {
                        value.clone()
                    }
                }
                _ => value.clone(),
            },
            "integer" => match value {
                Value::Number(n) if n.is_i64() || n.is_u64() => value.clone(),
                Value::Number(n) if n.is_f64() => {
                    let flt = n.as_f64().unwrap();
                    Value::Number((flt.round() as i64).into())
                }
                Value::String(s) => {
                    let clean = s.trim_matches(|c: char| !c.is_numeric() && c != '-');
                    if let Ok(num) = clean.parse::<i64>() {
                        Value::Number(num.into())
                    } else {
                        value.clone()
                    }
                }
                _ => value.clone(),
            },
            "boolean" => match value {
                Value::Bool(_) => value.clone(),
                Value::String(s) => match s.trim().to_lowercase().as_str() {
                    "true" | "yes" | "1" | "in_stock" | "instock" => Value::Bool(true),
                    "false" | "no" | "0" | "out_of_stock" | "outofstock" => Value::Bool(false),
                    _ => value.clone(),
                },
                _ => value.clone(),
            },
            "string" => match value {
                Value::String(_) => value.clone(),
                Value::Number(n) => Value::String(n.to_string()),
                Value::Bool(b) => Value::String(b.to_string()),
                _ => value.clone(),
            },
            _ => value.clone(),
        }
    }
}
