use crate::types::*;
use serde_json::Value;
use std::collections::HashMap;

pub fn analyze_json(json_str: &str, root_name: &str) -> Result<JsonSchema, Json2RustError> {
    let value: Value = serde_json::from_str(json_str)?;
    
    if json_str.trim().starts_with('[') {
        analyze_json_array(&value, root_name)
    } else {
        analyze_json_value(&value, root_name)
    }
}

fn analyze_json_array(value: &Value, root_name: &str) -> Result<JsonSchema, Json2RustError> {
    if let Value::Array(arr) = value {
        if arr.is_empty() {
            return Ok(JsonSchema {
                name: root_name.to_string(),
                json_type: JsonType::Array(Box::new(JsonType::Object(HashMap::new()))),
                optional: false,
            });
        }

        let mut merged_schema = None;
        for item in arr {
            let item_schema = analyze_json_value(item, &format!("{}Item", root_name))?;
            merged_schema = Some(match merged_schema {
                None => item_schema,
                Some(existing) => merge_schemas(existing, item_schema)?,
            });
        }

        if let Some(schema) = merged_schema {
            Ok(JsonSchema {
                name: root_name.to_string(),
                json_type: JsonType::Array(Box::new(schema.json_type)),
                optional: false,
            })
        } else {
            Ok(JsonSchema {
                name: root_name.to_string(),
                json_type: JsonType::Array(Box::new(JsonType::Object(HashMap::new()))),
                optional: false,
            })
        }
    } else {
        analyze_json_value(value, root_name)
    }
}

fn analyze_json_value(value: &Value, name: &str) -> Result<JsonSchema, Json2RustError> {
    let json_type = match value {
        Value::Null => JsonType::Null,
        Value::Bool(_) => JsonType::Boolean,
        Value::Number(_) => JsonType::Number,
        Value::String(_) => JsonType::String,
        Value::Array(arr) => {
            if arr.is_empty() {
                JsonType::Array(Box::new(JsonType::Null))
            } else {
                let mut element_type = None;
                for item in arr {
                    let item_schema = analyze_json_value(item, &format!("{}Item", name))?;
                    element_type = Some(match element_type {
                        None => item_schema.json_type,
                        Some(existing) => merge_types(existing, item_schema.json_type)?,
                    });
                }
                JsonType::Array(Box::new(element_type.unwrap_or(JsonType::Null)))
            }
        }
        Value::Object(obj) => {
            let mut fields = HashMap::new();
            for (key, val) in obj {
                let field_schema = analyze_json_value(val, &to_pascal_case(key))?;
                fields.insert(key.clone(), field_schema.json_type);
            }
            JsonType::Object(fields)
        }
    };

    Ok(JsonSchema {
        name: name.to_string(),
        json_type,
        optional: false,
    })
}

fn merge_schemas(schema1: JsonSchema, schema2: JsonSchema) -> Result<JsonSchema, Json2RustError> {
    Ok(JsonSchema {
        name: schema1.name,
        json_type: merge_types(schema1.json_type, schema2.json_type)?,
        optional: schema1.optional || schema2.optional,
    })
}

fn merge_types(type1: JsonType, type2: JsonType) -> Result<JsonType, Json2RustError> {
    match (type1, type2) {
        (JsonType::Null, other) | (other, JsonType::Null) => Ok(other),
        (JsonType::String, JsonType::String) => Ok(JsonType::String),
        (JsonType::Number, JsonType::Number) => Ok(JsonType::Number),
        (JsonType::Boolean, JsonType::Boolean) => Ok(JsonType::Boolean),
        (JsonType::Array(elem1), JsonType::Array(elem2)) => {
            let merged_elem = merge_types(*elem1, *elem2)?;
            Ok(JsonType::Array(Box::new(merged_elem)))
        }
        (JsonType::Object(fields1), JsonType::Object(fields2)) => {
            let mut merged_fields = fields1;
            for (key, type2) in fields2 {
                if let Some(type1) = merged_fields.get(&key) {
                    let merged_type = merge_types(type1.clone(), type2)?;
                    merged_fields.insert(key, merged_type);
                } else {
                    merged_fields.insert(key, type2);
                }
            }
            Ok(JsonType::Object(merged_fields))
        }
        (JsonType::String, JsonType::Number) | (JsonType::Number, JsonType::String) => {
            Ok(JsonType::String)
        }
        _ => Ok(JsonType::String),
    }
}

pub fn to_pascal_case(s: &str) -> String {
    s.split(['_', '-', ' '])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
            }
        })
        .collect()
}

pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut prev_lowercase = false;
    
    for (i, ch) in s.char_indices() {
        if ch.is_uppercase() {
            if i > 0 && prev_lowercase {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
            prev_lowercase = false;
        } else {
            result.push(ch);
            prev_lowercase = ch.is_lowercase();
        }
    }
    
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_simple_object() {
        let json = r#"{"name": "John", "age": 30, "active": true}"#;
        let schema = analyze_json(json, "Person").unwrap();
        
        assert_eq!(schema.name, "Person");
        if let JsonType::Object(fields) = schema.json_type {
            assert_eq!(fields.len(), 3);
            assert!(matches!(fields.get("name"), Some(JsonType::String)));
            assert!(matches!(fields.get("age"), Some(JsonType::Number)));
            assert!(matches!(fields.get("active"), Some(JsonType::Boolean)));
        } else {
            panic!("Expected object type");
        }
    }

    #[test]
    fn test_analyze_array() {
        let json = r#"[{"id": 1}, {"id": 2}]"#;
        let schema = analyze_json(json, "Items").unwrap();
        
        assert_eq!(schema.name, "Items");
        if let JsonType::Array(elem_type) = schema.json_type {
            if let JsonType::Object(fields) = *elem_type {
                assert!(matches!(fields.get("id"), Some(JsonType::Number)));
            } else {
                panic!("Expected object element type");
            }
        } else {
            panic!("Expected array type");
        }
    }

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("first_name"), "FirstName");
        assert_eq!(to_pascal_case("user-id"), "UserId");
        assert_eq!(to_pascal_case("API_KEY"), "ApiKey");
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("FirstName"), "first_name");
        assert_eq!(to_snake_case("UserId"), "user_id");
        assert_eq!(to_snake_case("APIKey"), "apikey");
    }
}