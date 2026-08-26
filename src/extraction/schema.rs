use crate::error::CrawlerError;
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaType {
    Object,
    Array,
    String,
    Number,
    Integer,
    Boolean,
    Null,
}

impl std::str::FromStr for SchemaType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "object" => Ok(SchemaType::Object),
            "array" => Ok(SchemaType::Array),
            "string" => Ok(SchemaType::String),
            "number" => Ok(SchemaType::Number),
            "integer" => Ok(SchemaType::Integer),
            "boolean" | "bool" => Ok(SchemaType::Boolean),
            "null" => Ok(SchemaType::Null),
            _ => Err(format!("Unsupported schema type: {s}")),
        }
    }
}

pub struct SchemaValidator;

impl SchemaValidator {
    /// Validates the structure, depth, and property count of a JSON schema against configuration limits.
    pub fn validate_schema(
        schema: &Value,
        max_depth: usize,
        max_properties: usize,
    ) -> Result<(), CrawlerError> {
        if !schema.is_object() {
            return Err(CrawlerError::InvalidExtractionSchema(
                "Root schema must be a JSON object".to_string(),
            ));
        }

        let mut property_count = 0;
        Self::validate_subschema(schema, 0, max_depth, &mut property_count, max_properties)?;

        Ok(())
    }

    fn validate_subschema(
        schema: &Value,
        current_depth: usize,
        max_depth: usize,
        property_count: &mut usize,
        max_properties: usize,
    ) -> Result<(), CrawlerError> {
        if current_depth > max_depth {
            return Err(CrawlerError::SchemaTooDeep {
                depth: current_depth,
                max_depth,
            });
        }

        if *property_count > max_properties {
            return Err(CrawlerError::SchemaTooLarge {
                properties: *property_count,
                max_properties,
            });
        }

        let obj = match schema.as_object() {
            Some(o) => o,
            None => {
                return Err(CrawlerError::InvalidExtractionSchema(
                    "Subschema must be a JSON object".to_string(),
                ));
            }
        };

        // Validate type field if present
        if let Some(type_val) = obj.get("type") {
            match type_val {
                Value::String(s) => {
                    let _ = s.parse::<SchemaType>().map_err(|e| {
                        CrawlerError::InvalidExtractionSchema(format!("Invalid 'type': {e}"))
                    })?;
                }
                Value::Array(arr) => {
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            let _ = s.parse::<SchemaType>().map_err(|e| {
                                CrawlerError::InvalidExtractionSchema(format!(
                                    "Invalid array item in 'type': {e}"
                                ))
                            })?;
                        } else {
                            return Err(CrawlerError::InvalidExtractionSchema(
                                "'type' array items must be strings".to_string(),
                            ));
                        }
                    }
                }
                _ => {
                    return Err(CrawlerError::InvalidExtractionSchema(
                        "'type' must be a string or array of strings".to_string(),
                    ));
                }
            }
        }

        // Validate properties if object
        if let Some(props) = obj.get("properties") {
            let props_obj = props.as_object().ok_or_else(|| {
                CrawlerError::InvalidExtractionSchema(
                    "'properties' must be a JSON object".to_string(),
                )
            })?;

            for (_, prop_schema) in props_obj {
                *property_count += 1;
                if *property_count > max_properties {
                    return Err(CrawlerError::SchemaTooLarge {
                        properties: *property_count,
                        max_properties,
                    });
                }
                Self::validate_subschema(
                    prop_schema,
                    current_depth + 1,
                    max_depth,
                    property_count,
                    max_properties,
                )?;
            }
        }

        // Validate items if array
        if let Some(items) = obj.get("items") {
            match items {
                Value::Object(_) => {
                    Self::validate_subschema(
                        items,
                        current_depth + 1,
                        max_depth,
                        property_count,
                        max_properties,
                    )?;
                }
                Value::Array(item_schemas) => {
                    for item_schema in item_schemas {
                        Self::validate_subschema(
                            item_schema,
                            current_depth + 1,
                            max_depth,
                            property_count,
                            max_properties,
                        )?;
                    }
                }
                Value::Bool(_) => {}
                _ => {
                    return Err(CrawlerError::InvalidExtractionSchema(
                        "'items' must be a schema object or array of schema objects".to_string(),
                    ));
                }
            }
        }

        // Validate required field
        if let Some(req) = obj.get("required") {
            if !req.is_array() {
                return Err(CrawlerError::InvalidExtractionSchema(
                    "'required' must be an array of property name strings".to_string(),
                ));
            }
        }

        // Validate enum
        if let Some(enm) = obj.get("enum") {
            if !enm.is_array() {
                return Err(CrawlerError::InvalidExtractionSchema(
                    "'enum' must be an array".to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Canonicalizes a JSON value (recursively sorting object keys) so hash identity is deterministic.
pub fn canonicalize_schema(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted_keys: Vec<_> = map.keys().collect();
            sorted_keys.sort();

            let mut new_map = Map::new();
            for key in sorted_keys {
                if let Some(val) = map.get(key) {
                    new_map.insert(key.clone(), canonicalize_schema(val));
                }
            }
            Value::Object(new_map)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(canonicalize_schema).collect()),
        _ => value.clone(),
    }
}

/// Computes a deterministic BLAKE3 hash of a canonicalized JSON schema.
pub fn compute_schema_hash(schema: &Value) -> String {
    let canonical = canonicalize_schema(schema);
    let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
    let hash = blake3::hash(&bytes);
    hash.to_hex().to_string()
}
