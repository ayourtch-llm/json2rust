use crate::types::*;
use crate::analyzer::{to_pascal_case, to_snake_case};
use crate::parser::calculate_struct_similarity;
use std::collections::HashMap;

const SIMILARITY_THRESHOLD: f64 = 0.6;

pub fn generate_rust_structs(
    schema: &JsonSchema,
    existing_structs: &[ExistingStruct],
) -> Result<Vec<RustStruct>, Json2RustError> {
    generate_rust_structs_with_strategy(schema, existing_structs, &MergeStrategy::Optional)
}

pub fn generate_rust_structs_with_strategy(
    schema: &JsonSchema,
    existing_structs: &[ExistingStruct],
    merge_strategy: &MergeStrategy,
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
                merge_strategy,
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
            generate_struct_from_schema(schema, existing_structs, &mut structs, &mut generated_names, merge_strategy)?;
        }
    }
    
    Ok(structs)
}

fn generate_struct_from_schema(
    schema: &JsonSchema,
    existing_structs: &[ExistingStruct],
    structs: &mut Vec<RustStruct>,
    generated_names: &mut HashMap<String, usize>,
    merge_strategy: &MergeStrategy,
) -> Result<String, Json2RustError> {
    match &schema.json_type {
        JsonType::Object(fields) => {
            let struct_name = ensure_unique_name(&schema.name, generated_names);
            let rust_fields = generate_fields_from_object(
                fields,
                existing_structs,
                structs,
                generated_names,
                merge_strategy,
            )?;
            
            let rust_struct = if let Some(existing) = find_compatible_struct(&rust_fields, existing_structs) {
                extend_existing_struct(existing, rust_fields, merge_strategy)
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
                merge_strategy,
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
    merge_strategy: &MergeStrategy,
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
            merge_strategy,
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

fn extend_existing_struct(existing: &ExistingStruct, new_fields: Vec<RustField>, merge_strategy: &MergeStrategy) -> RustStruct {
    // Order-independent field classification
    let classification = classify_fields_for_extension(existing, &new_fields);
    
    match merge_strategy {
        MergeStrategy::Optional => extend_with_optional_fields(existing, classification),
        MergeStrategy::Enum => extend_with_enum_fields(existing, classification),
        MergeStrategy::Hybrid => extend_with_hybrid_fields(existing, classification),
    }
}

fn extend_with_optional_fields(existing: &ExistingStruct, classification: FieldClassification) -> RustStruct {
    let mut fields = Vec::new();
    
    // Add common fields first (mandatory with compatible types)
    for field in classification.common_fields {
        fields.push(field);
    }
    
    // Add old-only fields as optional (for backward compatibility)
    for mut field in classification.old_only_fields {
        if !field.type_name.starts_with("Option<") {
            field.type_name = format!("Option<{}>", field.type_name);
            field.is_optional = true;
        }
        fields.push(field);
    }
    
    // Add new-only fields as optional (for backward compatibility)
    for mut field in classification.new_only_fields {
        if !field.type_name.starts_with("Option<") {
            field.type_name = format!("Option<{}>", field.type_name);
            field.is_optional = true;
        }
        fields.push(field);
    }
    
    RustStruct {
        name: existing.name.clone(),
        fields,
        derives: vec!["Debug".to_string(), "Clone".to_string(), "Serialize".to_string(), "Deserialize".to_string()],
        is_optional: false,
    }
}

fn extend_with_enum_fields(existing: &ExistingStruct, classification: FieldClassification) -> RustStruct {
    let mut fields = Vec::new();
    
    // Add common fields first (mandatory with compatible types)
    for field in classification.common_fields {
        fields.push(field);
    }
    
    // Create enum for conflicting field groups if any exist
    if !classification.old_only_fields.is_empty() || !classification.new_only_fields.is_empty() {
        let enum_field = create_schema_variant_enum(&existing.name, &classification.old_only_fields, &classification.new_only_fields);
        fields.push(enum_field);
    }
    
    RustStruct {
        name: existing.name.clone(),
        fields,
        derives: vec!["Debug".to_string(), "Clone".to_string(), "Serialize".to_string(), "Deserialize".to_string()],
        is_optional: false,
    }
}

fn extend_with_hybrid_fields(existing: &ExistingStruct, classification: FieldClassification) -> RustStruct {
    let mut fields = Vec::new();
    
    // Add common fields first (mandatory with compatible types)
    for field in classification.common_fields {
        fields.push(field);
    }
    
    // For hybrid approach:
    // - If there are many conflicting fields (>3), use enum
    // - If there are few conflicting fields (<=3), use optional
    let total_conflicting = classification.old_only_fields.len() + classification.new_only_fields.len();
    
    if total_conflicting > 3 {
        // Use enum for large field groups
        let enum_field = create_schema_variant_enum(&existing.name, &classification.old_only_fields, &classification.new_only_fields);
        fields.push(enum_field);
    } else {
        // Use optional for small field groups
        for mut field in classification.old_only_fields {
            if !field.type_name.starts_with("Option<") {
                field.type_name = format!("Option<{}>", field.type_name);
                field.is_optional = true;
            }
            fields.push(field);
        }
        
        for mut field in classification.new_only_fields {
            if !field.type_name.starts_with("Option<") {
                field.type_name = format!("Option<{}>", field.type_name);
                field.is_optional = true;
            }
            fields.push(field);
        }
    }
    
    RustStruct {
        name: existing.name.clone(),
        fields,
        derives: vec!["Debug".to_string(), "Clone".to_string(), "Serialize".to_string(), "Deserialize".to_string()],
        is_optional: false,
    }
}

fn create_schema_variant_enum(struct_name: &str, _old_fields: &[RustField], _new_fields: &[RustField]) -> RustField {
    // Create enum type name
    let enum_name = format!("{}Variant", struct_name);
    
    // For now, create a simple enum field
    // In a full implementation, we'd generate the actual enum type
    RustField {
        name: "schema_variant".to_string(),
        type_name: enum_name,
        is_optional: false,
        serde_rename: None,
    }
}

#[derive(Debug)]
struct FieldClassification {
    common_fields: Vec<RustField>,     // In both schemas - mandatory
    old_only_fields: Vec<RustField>,   // Only in old schema - mandatory
    new_only_fields: Vec<RustField>,   // Only in new schema - mandatory
}

fn classify_fields_for_extension(existing: &ExistingStruct, new_fields: &[RustField]) -> FieldClassification {
    let mut common_fields = Vec::new();
    let mut old_only_fields = Vec::new();
    let mut new_only_fields = Vec::new();
    
    let new_field_map: HashMap<String, &RustField> = new_fields
        .iter()
        .map(|f| (f.name.clone(), f))
        .collect();
    
    // Process existing fields
    for (existing_field_name, existing_field_type) in &existing.fields {
        if let Some(new_field) = new_field_map.get(existing_field_name) {
            // Common field - exists in both schemas
            let compatible_type = get_compatible_type(existing_field_type, &new_field.type_name);
            common_fields.push(RustField {
                name: existing_field_name.clone(),
                type_name: compatible_type,
                is_optional: new_field.is_optional || existing_field_type.starts_with("Option<"),
                serde_rename: new_field.serde_rename.clone(),
            });
        } else {
            // Old-only field - exists only in existing schema
            // Keep as mandatory to preserve original contract
            old_only_fields.push(RustField {
                name: existing_field_name.clone(),
                type_name: existing_field_type.clone(),
                is_optional: existing_field_type.starts_with("Option<"),
                serde_rename: None,
            });
        }
    }
    
    // Process new fields that don't exist in existing schema
    for new_field in new_fields {
        if !existing.fields.contains_key(&new_field.name) {
            // New-only field - exists only in new schema
            // Keep as mandatory to preserve new contract
            new_only_fields.push(new_field.clone());
        }
    }
    
    eprintln!("🔍 Field classification: common={}, old-only={}, new-only={}", 
             common_fields.len(), old_only_fields.len(), new_only_fields.len());
    
    FieldClassification {
        common_fields,
        old_only_fields,
        new_only_fields,
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
    generate_code_with_preservation_and_strategy(structs, original_code, &MergeStrategy::Optional)
}

pub fn generate_code_with_preservation_and_strategy(
    structs: &[RustStruct],
    original_code: Option<&str>,
    merge_strategy: &MergeStrategy,
) -> Result<String, Json2RustError> {
    if let Some(original) = original_code {
        generate_code_preserving_original(structs, original, merge_strategy)
    } else {
        generate_code(structs)
    }
}

fn generate_code_preserving_original(
    new_structs: &[RustStruct],
    original_code: &str,
    merge_strategy: &MergeStrategy,
) -> Result<String, Json2RustError> {
    use syn::{File, Item, spanned::Spanned};
    
    let ast: File = syn::parse_str(original_code)
        .map_err(|e| Json2RustError::RustParsing(format!("Failed to parse original code: {}", e)))?;
    
    // Create a map of new structs by name for quick lookup
    let new_struct_map: std::collections::HashMap<String, &RustStruct> = new_structs
        .iter()
        .map(|s| (s.name.clone(), s))
        .collect();
    
    let mut result = String::new();
    let mut last_end = 0;
    
    // Find struct spans and sort them by position
    let mut struct_replacements = Vec::new();
    
    for item in &ast.items {
        if let Item::Struct(item_struct) = item {
            let struct_name = item_struct.ident.to_string();
            if let Some(new_struct) = new_struct_map.get(&struct_name) {
                // When user explicitly specifies a struct name, we should extend it regardless of similarity
                // The similarity threshold only applies for automatic struct detection
                eprintln!("🎯 Explicitly extending struct '{}' as requested by user", struct_name);
                let _span = item_struct.span();
                
                // Parse the existing struct to get its fields
                let existing_struct = parse_struct_from_item(item_struct)?;
                
                // Extend the existing struct with new fields from JSON
                let extended_struct = extend_existing_struct(&existing_struct, new_struct.fields.clone(), merge_strategy);
                
                // For proc_macro2::Span, we need to use a different approach
                // Let's find the struct boundaries by searching for the struct name
                let start_byte = find_struct_start(original_code, &struct_name)?;
                let end_byte = find_struct_end(original_code, start_byte)?;
                
                struct_replacements.push(StructReplacement {
                    start: start_byte,
                    end: end_byte,
                    new_struct: extended_struct,
                    name: struct_name.clone(),
                });
                
                eprintln!("🔄 Will replace struct '{}' with extended version", struct_name);
            }
        }
    }
    
    // Sort replacements by start position
    struct_replacements.sort_by_key(|r| r.start);
    
    // Process the file, preserving original text and replacing specific structs
    for replacement in struct_replacements {
        // Add original text up to this struct
        result.push_str(&original_code[last_end..replacement.start]);
        
        // Add the new struct code
        result.push_str(&generate_struct_code(&replacement.new_struct)?);
        
        last_end = replacement.end;
    }
    
    // Add remaining original text
    result.push_str(&original_code[last_end..]);
    
    // Add completely new structs that weren't in the original file
    for new_struct in new_structs {
        if !struct_exists_in_original(&ast, &new_struct.name) {
            result.push('\n');
            result.push_str(&generate_struct_code(new_struct)?);
            eprintln!("✨ Added new struct '{}'", new_struct.name);
        }
    }
    
    Ok(result)
}

struct StructReplacement {
    start: usize,
    end: usize,
    new_struct: RustStruct,
    #[allow(dead_code)]
    name: String,
}

fn find_struct_start(source: &str, struct_name: &str) -> Result<usize, Json2RustError> {
    // Simple approach: look for the start of the struct definition including derives
    let lines: Vec<&str> = source.lines().collect();
    let mut struct_line_idx = None;
    
    // Find the line with the struct definition
    for (i, line) in lines.iter().enumerate() {
        if line.trim().contains(&format!("struct {}", struct_name)) {
            struct_line_idx = Some(i);
            break;
        }
    }
    
    if let Some(struct_idx) = struct_line_idx {
        // Look backwards for derive attributes
        let mut start_idx = struct_idx;
        
        while start_idx > 0 {
            let prev_line = lines[start_idx - 1].trim();
            if prev_line.starts_with("#[derive(") || prev_line.starts_with("pub ") || prev_line.starts_with("//") || prev_line.is_empty() {
                start_idx -= 1;
            } else {
                break;
            }
        }
        
        // Calculate byte position
        let byte_pos = lines[..start_idx].iter().map(|l| l.len() + 1).sum::<usize>();
        Ok(byte_pos)
    } else {
        Err(Json2RustError::CodeGeneration(format!("Could not find struct {} in source", struct_name)))
    }
}

fn find_struct_end(source: &str, start: usize) -> Result<usize, Json2RustError> {
    let remaining = &source[start..];
    let mut brace_count = 0;
    let mut in_struct = false;
    
    for (i, ch) in remaining.char_indices() {
        match ch {
            '{' => {
                brace_count += 1;
                in_struct = true;
            }
            '}' => {
                brace_count -= 1;
                if in_struct && brace_count == 0 {
                    return Ok(start + i + 1);
                }
            }
            _ => {}
        }
    }
    
    Err(Json2RustError::CodeGeneration("Could not find end of struct".to_string()))
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

fn parse_struct_from_item(item_struct: &syn::ItemStruct) -> Result<ExistingStruct, Json2RustError> {
    let mut fields = HashMap::new();
    
    if let syn::Fields::Named(named_fields) = &item_struct.fields {
        for field in &named_fields.named {
            if let Some(field_name) = &field.ident {
                let field_type = type_to_string(&field.ty);
                fields.insert(field_name.to_string(), field_type);
            }
        }
    }
    
    Ok(ExistingStruct {
        name: item_struct.ident.to_string(),
        fields,
    })
}

fn type_to_string(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                match &segment.arguments {
                    syn::PathArguments::None => segment.ident.to_string(),
                    syn::PathArguments::AngleBracketed(args) => {
                        let inner_types: Vec<String> = args
                            .args
                            .iter()
                            .filter_map(|arg| {
                                if let syn::GenericArgument::Type(inner_ty) = arg {
                                    Some(type_to_string(inner_ty))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        
                        if inner_types.is_empty() {
                            segment.ident.to_string()
                        } else {
                            format!("{}<{}>", segment.ident, inner_types.join(", "))
                        }
                    }
                    _ => segment.ident.to_string(),
                }
            } else {
                "Unknown".to_string()
            }
        }
        _ => "Unknown".to_string(),
    }
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