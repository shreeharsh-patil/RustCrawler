use rustcrawl::extraction::schema::{canonicalize_schema, compute_schema_hash, SchemaValidator};
use rustcrawl::extraction::validation::JsonValidator;
use serde_json::json;

#[test]
fn test_schema_validator_valid_object_schema() {
    let schema = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "price": { "type": "number", "minimum": 0.0 },
            "in_stock": { "type": "boolean" },
            "tags": {
                "type": "array",
                "items": { "type": "string" }
            }
        },
        "required": ["name", "price"]
    });

    let result = SchemaValidator::validate_schema(&schema, 20, 500);
    assert!(result.is_ok());
}

#[test]
fn test_schema_validator_rejects_non_object() {
    let schema = json!("string_is_not_a_valid_root_schema");
    let result = SchemaValidator::validate_schema(&schema, 20, 500);
    assert!(result.is_err());
}

#[test]
fn test_schema_validator_depth_limit_exceeded() {
    // Construct schema with depth 6
    let mut schema = json!({ "type": "string" });
    for i in 0..6 {
        schema = json!({
            "type": "object",
            "properties": {
                format!("nested_{i}"): schema
            }
        });
    }

    // Max depth 4 should fail
    let result = SchemaValidator::validate_schema(&schema, 4, 500);
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("depth") || err_str.contains("exceeded"));
}

#[test]
fn test_schema_validator_properties_limit_exceeded() {
    let mut props = serde_json::Map::new();
    for i in 0..50 {
        props.insert(format!("prop_{i}"), json!({ "type": "string" }));
    }
    let schema = json!({
        "type": "object",
        "properties": props
    });

    // Max properties 30 should fail
    let result = SchemaValidator::validate_schema(&schema, 20, 30);
    assert!(result.is_err());
}

#[test]
fn test_schema_canonicalization_and_hashing() {
    let schema1 = json!({
        "type": "object",
        "properties": {
            "zebra": { "type": "string" },
            "apple": { "type": "number" },
            "middle": { "type": "boolean" }
        },
        "required": ["zebra", "apple"]
    });

    let schema2 = json!({
        "required": ["zebra", "apple"],
        "properties": {
            "apple": { "type": "number" },
            "middle": { "type": "boolean" },
            "zebra": { "type": "string" }
        },
        "type": "object"
    });

    let canonical1 = canonicalize_schema(&schema1);
    let canonical2 = canonicalize_schema(&schema2);
    assert_eq!(canonical1, canonical2);

    let hash1 = compute_schema_hash(&schema1);
    let hash2 = compute_schema_hash(&schema2);
    assert_eq!(hash1, hash2);
}

#[test]
fn test_json_validator_all_data_types() {
    let schema = json!({
        "type": "object",
        "properties": {
            "str_field": { "type": "string", "minLength": 2, "maxLength": 10 },
            "num_field": { "type": "number", "minimum": 10.0, "maximum": 100.0 },
            "int_field": { "type": "integer" },
            "bool_field": { "type": "boolean" },
            "enum_field": { "type": "string", "enum": ["active", "inactive", "pending"] },
            "arr_field": {
                "type": "array",
                "items": { "type": "string" }
            }
        },
        "required": ["str_field", "num_field", "bool_field"]
    });

    let valid_data = json!({
        "str_field": "Valid",
        "num_field": 42.5,
        "int_field": 7,
        "bool_field": true,
        "enum_field": "active",
        "arr_field": ["alpha", "beta"]
    });
    assert!(JsonValidator::validate(&valid_data, &schema).is_ok());

    // Test missing required field
    let invalid_missing = json!({
        "str_field": "Valid",
        "bool_field": true
    });
    assert!(JsonValidator::validate(&invalid_missing, &schema).is_err());

    // Test enum mismatch
    let invalid_enum = json!({
        "str_field": "Valid",
        "num_field": 50.0,
        "bool_field": false,
        "enum_field": "invalid_option"
    });
    assert!(JsonValidator::validate(&invalid_enum, &schema).is_err());

    // Test minimum number violation
    let invalid_min = json!({
        "str_field": "Valid",
        "num_field": 5.0,
        "bool_field": false
    });
    assert!(JsonValidator::validate(&invalid_min, &schema).is_err());
}

#[test]
fn test_json_validator_additional_properties_false() {
    let schema = json!({
        "type": "object",
        "properties": {
            "allowed_key": { "type": "string" }
        },
        "additionalProperties": false
    });

    let valid = json!({ "allowed_key": "hello" });
    assert!(JsonValidator::validate(&valid, &schema).is_ok());

    let invalid = json!({
        "allowed_key": "hello",
        "disallowed_key": "surprise"
    });
    assert!(JsonValidator::validate(&invalid, &schema).is_err());
}
