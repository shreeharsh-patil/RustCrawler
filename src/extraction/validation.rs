use serde_json::Value;

pub struct JsonValidator;

impl JsonValidator {
    /// Validates a JSON value against a JSON schema.
    /// Returns Ok(()) if valid, or Err(Vec<String>) with error descriptions.
    pub fn validate(value: &Value, schema: &Value) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        Self::validate_node(value, schema, "$", &mut errors);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn validate_node(value: &Value, schema: &Value, path: &str, errors: &mut Vec<String>) {
        let schema_obj = match schema.as_object() {
            Some(o) => o,
            None => return,
        };

        // 1. Check type constraint
        if let Some(type_val) = schema_obj.get("type") {
            let mut type_matched = false;
            let type_names: Vec<&str> = match type_val {
                Value::String(s) => vec![s.as_str()],
                Value::Array(arr) => arr.iter().filter_map(|v| v.as_str()).collect(),
                _ => Vec::new(),
            };

            for type_name in type_names {
                match type_name {
                    "object" if value.is_object() => type_matched = true,
                    "array" if value.is_array() => type_matched = true,
                    "string" if value.is_string() => type_matched = true,
                    "number" if value.is_number() => type_matched = true,
                    "integer"
                        if value.is_i64()
                            || value.is_u64()
                            || (value.is_f64() && value.as_f64().unwrap().fract() == 0.0) =>
                    {
                        type_matched = true
                    }
                    "boolean" if value.is_boolean() => type_matched = true,
                    "null" if value.is_null() => type_matched = true,
                    _ => {}
                }
            }

            if !type_matched {
                errors.push(format!(
                    "Field '{path}' expected type {:?}, found {}",
                    type_val,
                    json_type_name(value)
                ));
                return;
            }
        }

        // 2. Check enum constraint
        if let Some(enum_val) = schema_obj.get("enum").and_then(|e| e.as_array()) {
            if !enum_val.contains(value) {
                errors.push(format!(
                    "Field '{path}' value '{value}' is not in allowed enum: {:?}",
                    enum_val
                ));
            }
        }

        // 3. Object validations
        if let Some(map) = value.as_object() {
            // Required properties
            if let Some(req_arr) = schema_obj.get("required").and_then(|r| r.as_array()) {
                for req in req_arr {
                    if let Some(req_name) = req.as_str() {
                        if !map.contains_key(req_name) || map.get(req_name).unwrap().is_null() {
                            errors.push(format!("Missing required field '{path}.{req_name}'"));
                        }
                    }
                }
            }

            // Properties
            if let Some(props) = schema_obj.get("properties").and_then(|p| p.as_object()) {
                for (prop_name, prop_schema) in props {
                    if let Some(prop_val) = map.get(prop_name) {
                        let child_path = format!("{path}.{prop_name}");
                        Self::validate_node(prop_val, prop_schema, &child_path, errors);
                    }
                }

                // Additional properties
                if let Some(add_props) = schema_obj.get("additionalProperties") {
                    if let Some(false) = add_props.as_bool() {
                        for key in map.keys() {
                            if !props.contains_key(key) {
                                errors
                                    .push(format!("Disallowed additional property '{path}.{key}'"));
                            }
                        }
                    }
                }
            }
        }

        // 4. Array validations
        if let Some(arr) = value.as_array() {
            if let Some(items_schema) = schema_obj.get("items") {
                for (idx, item) in arr.iter().enumerate() {
                    let child_path = format!("{path}[{idx}]");
                    Self::validate_node(item, items_schema, &child_path, errors);
                }
            }
        }

        // 5. Numeric range validations
        if let Some(num) = value.as_f64() {
            if let Some(min) = schema_obj.get("minimum").and_then(|m| m.as_f64()) {
                if num < min {
                    errors.push(format!(
                        "Field '{path}' value {num} is less than minimum {min}"
                    ));
                }
            }
            if let Some(max) = schema_obj.get("maximum").and_then(|m| m.as_f64()) {
                if num > max {
                    errors.push(format!(
                        "Field '{path}' value {num} is greater than maximum {max}"
                    ));
                }
            }
        }

        // 6. String length validations
        if let Some(s) = value.as_str() {
            if let Some(min_len) = schema_obj.get("minLength").and_then(|m| m.as_u64()) {
                if (s.len() as u64) < min_len {
                    errors.push(format!(
                        "Field '{path}' length {} is less than minLength {min_len}",
                        s.len()
                    ));
                }
            }
            if let Some(max_len) = schema_obj.get("maxLength").and_then(|m| m.as_u64()) {
                if (s.len() as u64) > max_len {
                    errors.push(format!(
                        "Field '{path}' length {} exceeds maxLength {max_len}",
                        s.len()
                    ));
                }
            }
        }
    }
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
