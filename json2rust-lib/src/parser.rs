use crate::types::*;
use std::collections::HashMap;
use syn::{File, Item, ItemStruct, Type, TypePath, Fields, FieldsNamed};

pub fn parse_existing_structs(rust_code: &str) -> Result<Vec<ExistingStruct>, Json2RustError> {
    let ast: File = syn::parse_str(rust_code)
        .map_err(|e| Json2RustError::RustParsing(format!("Failed to parse Rust code: {}", e)))?;
    
    let mut structs = Vec::new();
    
    for item in ast.items {
        if let Item::Struct(item_struct) = item {
            let existing_struct = parse_struct_item(&item_struct)?;
            structs.push(existing_struct);
        }
    }
    
    Ok(structs)
}

fn parse_struct_item(item_struct: &ItemStruct) -> Result<ExistingStruct, Json2RustError> {
    let name = item_struct.ident.to_string();
    let mut fields = HashMap::new();
    
    if let Fields::Named(FieldsNamed { named, .. }) = &item_struct.fields {
        for field in named {
            let field_name = field.ident.as_ref()
                .ok_or_else(|| Json2RustError::RustParsing("Field missing name".to_string()))?
                .to_string();
            
            let field_type = extract_type_string(&field.ty)?;
            fields.insert(field_name, field_type);
        }
    }
    
    Ok(ExistingStruct { name, fields })
}

fn extract_type_string(ty: &Type) -> Result<String, Json2RustError> {
    match ty {
        Type::Path(TypePath { path, .. }) => {
            let segments: Vec<String> = path.segments.iter()
                .map(|seg| seg.ident.to_string())
                .collect();
            Ok(segments.join("::"))
        }
        Type::Reference(type_ref) => {
            let inner_type = extract_type_string(&type_ref.elem)?;
            Ok(format!("&{}", inner_type))
        }
        _ => Ok("Unknown".to_string()),
    }
}

pub fn calculate_struct_similarity(existing: &ExistingStruct, new_fields: &HashMap<String, String>) -> f64 {
    if existing.fields.is_empty() && new_fields.is_empty() {
        return 1.0;
    }
    
    let total_fields = (existing.fields.len() + new_fields.len()) as f64;
    let mut common_fields = 0;
    let mut compatible_fields = 0;
    
    for (field_name, new_type) in new_fields {
        if let Some(existing_type) = existing.fields.get(field_name) {
            common_fields += 1;
            if are_types_compatible(existing_type, new_type) {
                compatible_fields += 1;
            }
        }
    }
    
    let field_overlap = (common_fields as f64) / total_fields;
    let type_compatibility = if common_fields > 0 {
        (compatible_fields as f64) / (common_fields as f64)
    } else {
        0.0
    };
    
    (field_overlap + type_compatibility) / 2.0
}

fn are_types_compatible(existing_type: &str, new_type: &str) -> bool {
    if existing_type == new_type {
        return true;
    }
    
    let existing_optional = existing_type.starts_with("Option<");
    let new_optional = new_type.starts_with("Option<");
    
    if existing_optional && !new_optional {
        let inner_existing = extract_option_inner(existing_type);
        return inner_existing == new_type;
    }
    
    if !existing_optional && new_optional {
        let inner_new = extract_option_inner(new_type);
        return existing_type == inner_new;
    }
    
    match (existing_type, new_type) {
        ("String", "i64") | ("i64", "String") => true,
        ("String", "f64") | ("f64", "String") => true,
        ("i64", "f64") | ("f64", "i64") => true,
        _ => false,
    }
}

fn extract_option_inner(option_type: &str) -> &str {
    if option_type.starts_with("Option<") && option_type.ends_with('>') {
        &option_type[7..option_type.len()-1]
    } else {
        option_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_struct() {
        let code = r#"
            struct Person {
                name: String,
                age: i32,
            }
        "#;
        
        let structs = parse_existing_structs(code).unwrap();
        assert_eq!(structs.len(), 1);
        assert_eq!(structs[0].name, "Person");
        assert_eq!(structs[0].fields.get("name"), Some(&"String".to_string()));
        assert_eq!(structs[0].fields.get("age"), Some(&"i32".to_string()));
    }

    #[test]
    fn test_calculate_struct_similarity() {
        let existing = ExistingStruct {
            name: "Person".to_string(),
            fields: {
                let mut fields = HashMap::new();
                fields.insert("name".to_string(), "String".to_string());
                fields.insert("age".to_string(), "i32".to_string());
                fields
            },
        };
        
        let mut new_fields = HashMap::new();
        new_fields.insert("name".to_string(), "String".to_string());
        new_fields.insert("age".to_string(), "i32".to_string());
        
        let similarity = calculate_struct_similarity(&existing, &new_fields);
        assert!(similarity > 0.7);
    }

    #[test]
    fn test_are_types_compatible() {
        assert!(are_types_compatible("String", "String"));
        assert!(are_types_compatible("Option<String>", "String"));
        assert!(are_types_compatible("String", "Option<String>"));
        assert!(are_types_compatible("String", "i64"));
        assert!(!are_types_compatible("bool", "String"));
    }
}