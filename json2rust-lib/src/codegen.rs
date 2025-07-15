use crate::types::*;
use crate::analyzer::{to_pascal_case, to_snake_case};
use crate::parser::calculate_struct_similarity;
use std::collections::HashMap;

const SIMILARITY_THRESHOLD: f64 = 0.6;

pub fn generate_rust_structs(
    schema: &JsonSchema,
    existing_structs: &[ExistingStruct],
) -> Result<Vec<RustStruct>, Json2RustError> {
    let mut structs = Vec::new();
    let mut generated_names = HashMap::new();
    
    match &schema.json_type {
        JsonType::Array(element_type) => {
            // For arrays, try to find existing struct with singular name first
            let element_name = get_singular_name(&schema.name);
            let element_type_name = generate_struct_from_schema(
                &JsonSchema {
                    name: element_name,
                    json_type: (**element_type).clone(),
                    optional: false,
                },
                existing_structs,
                &mut structs,
                &mut generated_names,
            )?;
            
            let root_struct = RustStruct {
                name: schema.name.clone(),
                fields: vec![RustField {
                    name: "items".to_string(),
                    type_name: format!("Vec<{}>", element_type_name),
                    is_optional: false,
                    serde_rename: None,
                }],
                derives: vec!["Debug".to_string(), "Clone".to_string(), "Serialize".to_string(), "Deserialize".to_string()],
                is_optional: false,
            };
            structs.push(root_struct);
        }
        _ => {
            generate_struct_from_schema(schema, existing_structs, &mut structs, &mut generated_names)?;
        }
    }
    
    Ok(structs)
}

fn generate_struct_from_schema(
    schema: &JsonSchema,
    existing_structs: &[ExistingStruct],
    structs: &mut Vec<RustStruct>,
    generated_names: &mut HashMap<String, usize>,
) -> Result<String, Json2RustError> {
    match &schema.json_type {
        JsonType::Object(fields) => {
            let struct_name = ensure_unique_name(&schema.name, generated_names);
            let rust_fields = generate_fields_from_object(
                fields,
                existing_structs,
                structs,
                generated_names,
            )?;
            
            let rust_struct = if let Some(existing) = find_compatible_struct(&rust_fields, existing_structs) {
                extend_existing_struct(existing, rust_fields)
            } else {
                RustStruct {
                    name: struct_name.clone(),
                    fields: rust_fields,
                    derives: vec!["Debug".to_string(), "Clone".to_string(), "Serialize".to_string(), "Deserialize".to_string()],
                    is_optional: schema.optional,
                }
            };
            
            structs.push(rust_struct);
            Ok(struct_name)
        }
        JsonType::Array(element_type) => {
            let element_type_name = generate_struct_from_schema(
                &JsonSchema {
                    name: format!("{}Item", schema.name),
                    json_type: (**element_type).clone(),
                    optional: false,
                },
                existing_structs,
                structs,
                generated_names,
            )?;
            Ok(format!("Vec<{}>", element_type_name))
        }
        JsonType::String => Ok("String".to_string()),
        JsonType::Number => Ok("f64".to_string()),
        JsonType::Boolean => Ok("bool".to_string()),
        JsonType::Null => Ok("Option<serde_json::Value>".to_string()),
    }
}

fn generate_fields_from_object(
    fields: &HashMap<String, JsonType>,
    existing_structs: &[ExistingStruct],
    structs: &mut Vec<RustStruct>,
    generated_names: &mut HashMap<String, usize>,
) -> Result<Vec<RustField>, Json2RustError> {
    let mut rust_fields = Vec::new();
    
    for (field_name, field_type) in fields {
        let field_type_name = generate_struct_from_schema(
            &JsonSchema {
                name: to_pascal_case(field_name),
                json_type: field_type.clone(),
                optional: false,
            },
            existing_structs,
            structs,
            generated_names,
        )?;
        
        let rust_field = RustField {
            name: to_snake_case(field_name),
            type_name: field_type_name,
            is_optional: matches!(field_type, JsonType::Null),
            serde_rename: if to_snake_case(field_name) != *field_name {
                Some(field_name.clone())
            } else {
                None
            },
        };
        
        rust_fields.push(rust_field);
    }
    
    Ok(rust_fields)
}

fn find_compatible_struct<'a>(
    new_fields: &[RustField],
    existing_structs: &'a [ExistingStruct],
) -> Option<&'a ExistingStruct> {
    let new_field_map: HashMap<String, String> = new_fields
        .iter()
        .map(|f| (f.name.clone(), f.type_name.clone()))
        .collect();
    
    existing_structs
        .iter()
        .find(|existing| {
            calculate_struct_similarity(existing, &new_field_map) >= SIMILARITY_THRESHOLD
        })
}

fn extend_existing_struct(existing: &ExistingStruct, new_fields: Vec<RustField>) -> RustStruct {
    let mut fields = Vec::new();
    let new_field_map: HashMap<String, &RustField> = new_fields
        .iter()
        .map(|f| (f.name.clone(), f))
        .collect();
    
    // First, add all existing fields in their original order
    for (existing_field_name, existing_field_type) in &existing.fields {
        if let Some(new_field) = new_field_map.get(existing_field_name) {
            // Field exists in both - use compatible type
            let compatible_type = get_compatible_type(existing_field_type, &new_field.type_name);
            fields.push(RustField {
                name: existing_field_name.clone(),
                type_name: compatible_type,
                is_optional: new_field.is_optional || existing_field_type.starts_with("Option<"),
                serde_rename: new_field.serde_rename.clone(),
            });
        } else {
            // Field only exists in existing struct - make it optional
            let optional_type = if existing_field_type.starts_with("Option<") {
                existing_field_type.clone() // Already optional
            } else {
                format!("Option<{}>", existing_field_type)
            };
            fields.push(RustField {
                name: existing_field_name.clone(),
                type_name: optional_type,
                is_optional: true,
                serde_rename: None,
            });
        }
    }
    
    // Then add new fields that don't exist in the existing struct
    for new_field in &new_fields {
        if !existing.fields.contains_key(&new_field.name) {
            fields.push(new_field.clone());
        }
    }
    
    RustStruct {
        name: existing.name.clone(),
        fields,
        derives: vec!["Debug".to_string(), "Clone".to_string(), "Serialize".to_string(), "Deserialize".to_string()],
        is_optional: false,
    }
}

fn get_compatible_type(existing_type: &str, new_type: &str) -> String {
    // If types are identical, use existing type
    if existing_type == new_type {
        return existing_type.to_string();
    }
    
    // Handle numeric type compatibility
    if is_numeric_type(existing_type) && new_type == "f64" {
        return existing_type.to_string(); // Prefer existing numeric type
    }
    
    // Handle Option types - if existing is Option<T> and new is T, keep as Option<T>
    if existing_type.starts_with("Option<") && !new_type.starts_with("Option<") {
        let inner_existing = extract_option_inner(existing_type);
        if inner_existing == new_type || (is_numeric_type(inner_existing) && new_type == "f64") {
            return existing_type.to_string(); // Keep existing Optional type
        }
    }
    
    if !existing_type.starts_with("Option<") && new_type.starts_with("Option<") {
        let inner_new = extract_option_inner(new_type);
        if existing_type == inner_new || (is_numeric_type(existing_type) && inner_new == "f64") {
            return new_type.to_string();
        }
    }
    
    // Default to new type if no compatibility found
    new_type.to_string()
}

fn is_numeric_type(type_name: &str) -> bool {
    matches!(type_name, "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | 
                        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | 
                        "f32" | "f64")
}

fn extract_option_inner(option_type: &str) -> &str {
    if option_type.starts_with("Option<") && option_type.ends_with('>') {
        &option_type[7..option_type.len()-1]
    } else {
        option_type
    }
}

fn get_singular_name(plural_name: &str) -> String {
    // Simple pluralization rules - can be enhanced
    if plural_name.ends_with("ies") {
        format!("{}y", &plural_name[..plural_name.len()-3])
    } else if plural_name.ends_with("s") && !plural_name.ends_with("ss") {
        plural_name[..plural_name.len()-1].to_string()
    } else {
        // If no clear plural pattern, use "Item" suffix
        format!("{}Item", plural_name)
    }
}

fn ensure_unique_name(base_name: &str, generated_names: &mut HashMap<String, usize>) -> String {
    let count = generated_names.entry(base_name.to_string()).or_insert(0);
    *count += 1;
    
    if *count == 1 {
        base_name.to_string()
    } else {
        format!("{}{}", base_name, count)
    }
}

pub fn generate_code(structs: &[RustStruct]) -> Result<String, Json2RustError> {
    let mut code = String::new();
    
    code.push_str("use serde::{Deserialize, Serialize};\n\n");
    
    for rust_struct in structs {
        code.push_str(&generate_struct_code(rust_struct)?);
        code.push('\n');
    }
    
    Ok(code)
}

pub fn generate_code_with_preservation(
    structs: &[RustStruct],
    original_code: Option<&str>,
) -> Result<String, Json2RustError> {
    if let Some(original) = original_code {
        generate_code_preserving_original(structs, original)
    } else {
        generate_code(structs)
    }
}

fn generate_code_preserving_original(
    new_structs: &[RustStruct],
    original_code: &str,
) -> Result<String, Json2RustError> {
    use syn::{File, Item};
    
    let ast: File = syn::parse_str(original_code)
        .map_err(|e| Json2RustError::RustParsing(format!("Failed to parse original code: {}", e)))?;
    
    // Create a map of new structs by name for quick lookup
    let new_struct_map: std::collections::HashMap<String, &RustStruct> = new_structs
        .iter()
        .map(|s| (s.name.clone(), s))
        .collect();
    
    let mut result = String::new();
    
    // Extract imports and other non-struct items first
    let mut imports = Vec::new();
    let mut other_items = Vec::new();
    let mut original_structs = Vec::new();
    
    for item in &ast.items {
        match item {
            Item::Use(_) => imports.push(item),
            Item::Struct(item_struct) => {
                let struct_name = item_struct.ident.to_string();
                if new_struct_map.contains_key(&struct_name) {
                    // This struct will be replaced
                    eprintln!("🔄 Replaced struct '{}' with extended version", struct_name);
                } else {
                    // This struct will be preserved
                    original_structs.push(item_struct);
                }
            }
            _ => other_items.push(item),
        }
    }
    
    // Generate the result by preserving structure
    for import in &imports {
        result.push_str(&quote::quote!(#import).to_string());
        result.push('\n');
    }
    
    if !imports.is_empty() {
        result.push('\n');
    }
    
    // Add preserved structs
    for original_struct in &original_structs {
        result.push_str(&quote::quote!(#original_struct).to_string());
        result.push('\n');
    }
    
    // Add replaced structs
    for new_struct in new_structs {
        if struct_exists_in_original(&ast, &new_struct.name) {
            result.push_str(&generate_struct_code(new_struct)?);
            result.push('\n');
        }
    }
    
    // Add other items
    for item in &other_items {
        result.push_str(&quote::quote!(#item).to_string());
        result.push('\n');
    }
    
    // Add completely new structs that weren't in the original file
    for new_struct in new_structs {
        if !struct_exists_in_original(&ast, &new_struct.name) {
            result.push_str(&generate_struct_code(new_struct)?);
            result.push('\n');
            eprintln!("✨ Added new struct '{}'", new_struct.name);
        }
    }
    
    Ok(result)
}

fn struct_exists_in_original(ast: &syn::File, name: &str) -> bool {
    ast.items.iter().any(|item| {
        if let syn::Item::Struct(item_struct) = item {
            item_struct.ident.to_string() == name
        } else {
            false
        }
    })
}


fn generate_struct_code(rust_struct: &RustStruct) -> Result<String, Json2RustError> {
    let mut code = String::new();
    
    let derives = rust_struct.derives.join(", ");
    code.push_str(&format!("#[derive({})]\n", derives));
    code.push_str(&format!("pub struct {} {{\n", rust_struct.name));
    
    for field in &rust_struct.fields {
        if let Some(rename) = &field.serde_rename {
            code.push_str(&format!("    #[serde(rename = \"{}\")]\n", rename));
        }
        
        if field.is_optional {
            code.push_str(&format!("    #[serde(skip_serializing_if = \"Option::is_none\")]\n"));
        }
        
        let field_type = if field.is_optional && !field.type_name.starts_with("Option<") {
            format!("Option<{}>", field.type_name)
        } else {
            field.type_name.clone()
        };
        
        code.push_str(&format!("    pub {}: {},\n", field.name, field_type));
    }
    
    code.push_str("}\n");
    
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_simple_struct() {
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), JsonType::String);
        fields.insert("age".to_string(), JsonType::Number);
        
        let schema = JsonSchema {
            name: "Person".to_string(),
            json_type: JsonType::Object(fields),
            optional: false,
        };
        
        let structs = generate_rust_structs(&schema, &[]).unwrap();
        assert_eq!(structs.len(), 1);
        assert_eq!(structs[0].name, "Person");
        assert_eq!(structs[0].fields.len(), 2);
    }

    #[test]
    fn test_generate_code() {
        let rust_struct = RustStruct {
            name: "Person".to_string(),
            fields: vec![
                RustField {
                    name: "name".to_string(),
                    type_name: "String".to_string(),
                    is_optional: false,
                    serde_rename: None,
                },
                RustField {
                    name: "age".to_string(),
                    type_name: "f64".to_string(),
                    is_optional: false,
                    serde_rename: None,
                },
            ],
            derives: vec!["Debug".to_string(), "Serialize".to_string(), "Deserialize".to_string()],
            is_optional: false,
        };
        
        let code = generate_code(&[rust_struct]).unwrap();
        assert!(code.contains("pub struct Person"));
        assert!(code.contains("pub name: String"));
        assert!(code.contains("pub age: f64"));
    }

    #[test]
    fn test_ensure_unique_name() {
        let mut generated_names = HashMap::new();
        
        assert_eq!(ensure_unique_name("Person", &mut generated_names), "Person");
        assert_eq!(ensure_unique_name("Person", &mut generated_names), "Person2");
        assert_eq!(ensure_unique_name("Person", &mut generated_names), "Person3");
    }
}