use crate::types::*;
use crate::analyzer::{to_pascal_case, to_snake_case};
use crate::parser::calculate_struct_similarity;
use std::collections::HashMap;

const SIMILARITY_THRESHOLD: f64 = 0.6;

pub fn generate_rust_structs(
    schema: &JsonSchema,
    existing_structs: &[ExistingStruct],
) -> Result<Vec<RustStruct>, Json2RustError> {
    let result = generate_rust_types_with_strategy(schema, existing_structs, &MergeStrategy::Optional)?;
    Ok(result.structs)
}

pub fn generate_rust_structs_with_strategy(
    schema: &JsonSchema,
    existing_structs: &[ExistingStruct],
    merge_strategy: &MergeStrategy,
) -> Result<Vec<RustStruct>, Json2RustError> {
    let result = generate_rust_types_with_strategy(schema, existing_structs, merge_strategy)?;
    Ok(result.structs)
}

#[derive(Debug, Clone)]
pub struct GeneratedTypes {
    pub structs: Vec<RustStruct>,
    pub enums: Vec<RustEnum>,
}

pub fn generate_rust_types_with_strategy(
    schema: &JsonSchema,
    existing_structs: &[ExistingStruct],
    merge_strategy: &MergeStrategy,
) -> Result<GeneratedTypes, Json2RustError> {
    let mut structs = Vec::new();
    let mut enums = Vec::new();
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
                &mut enums,
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
            generate_struct_from_schema(schema, existing_structs, &mut structs, &mut enums, &mut generated_names, merge_strategy)?;
        }
    }
    
    Ok(GeneratedTypes { structs, enums })
}

fn generate_struct_from_schema(
    schema: &JsonSchema,
    existing_structs: &[ExistingStruct],
    structs: &mut Vec<RustStruct>,
    enums: &mut Vec<RustEnum>,
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
                enums,
                generated_names,
                merge_strategy,
            )?;
            
            let rust_struct = if let Some(existing) = find_compatible_struct(&rust_fields, existing_structs) {
                extend_existing_struct(existing, rust_fields, enums, merge_strategy)
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
                enums,
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
    enums: &mut Vec<RustEnum>,
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
            enums,
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

fn extend_existing_struct(existing: &ExistingStruct, new_fields: Vec<RustField>, enums: &mut Vec<RustEnum>, merge_strategy: &MergeStrategy) -> RustStruct {
    match merge_strategy {
        MergeStrategy::Optional => {
            // Order-independent field classification (legacy method for optional strategy)
            let classification = classify_fields_for_extension(existing, &new_fields);
            extend_with_optional_fields(existing, classification)
        },
        MergeStrategy::Enum => extend_with_enum_fields(existing, new_fields, enums),
        MergeStrategy::Hybrid => {
            // Order-independent field classification (legacy method for hybrid strategy)
            let classification = classify_fields_for_extension(existing, &new_fields);
            extend_with_hybrid_fields(existing, classification, enums)
        },
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

fn extend_with_enum_fields(existing: &ExistingStruct, new_fields: Vec<RustField>, enums: &mut Vec<RustEnum>) -> RustStruct {
    // Use enhanced field classification that considers existing enums
    let classification = classify_fields_for_extension_with_enums(existing, &new_fields, enums);
    
    let mut fields = Vec::new();
    
    // Add common fields first (mandatory with compatible types)
    for field in classification.common_fields {
        fields.push(field);
    }
    
    // Check if struct already has a schema_variant field (existing enum)
    if let Some(existing_enum_type) = existing.fields.get("schema_variant") {
        eprintln!("🔍 Found existing schema_variant field of type: {}", existing_enum_type);
        
        // Find the existing enum in our enums collection
        if let Some(existing_enum) = enums.iter_mut().find(|e| e.name == *existing_enum_type) {
            eprintln!("🔄 Creating extended enum for existing enum: {}", existing_enum.name);
            
            // Create an extended enum with new variant for the new field combination
            let extended_enum = create_extended_enum(&existing.name, &classification.old_only_fields, &classification.new_only_fields, existing_enum);
            
            // Replace the existing enum with the extended one
            *existing_enum = extended_enum;
            
            // Use the existing enum field
            fields.push(RustField {
                name: "schema_variant".to_string(),
                type_name: existing_enum_type.clone(),
                is_optional: false,
                serde_rename: None,
            });
        } else {
            eprintln!("🔍 Existing enum '{}' not found in enums collection, creating new one", existing_enum_type);
            
            // Create enum for conflicting field groups if any exist
            if !classification.old_only_fields.is_empty() || !classification.new_only_fields.is_empty() {
                let enum_field = create_schema_variant_enum(&existing.name, &classification.old_only_fields, &classification.new_only_fields, enums);
                fields.push(enum_field);
            }
        }
    } else {
        // Create enum for conflicting field groups if any exist
        if !classification.old_only_fields.is_empty() || !classification.new_only_fields.is_empty() {
            let enum_field = create_schema_variant_enum(&existing.name, &classification.old_only_fields, &classification.new_only_fields, enums);
            fields.push(enum_field);
        }
    }
    
    RustStruct {
        name: existing.name.clone(),
        fields,
        derives: vec!["Debug".to_string(), "Clone".to_string(), "Serialize".to_string(), "Deserialize".to_string()],
        is_optional: false,
    }
}

fn extend_with_hybrid_fields(existing: &ExistingStruct, classification: FieldClassification, enums: &mut Vec<RustEnum>) -> RustStruct {
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
        let enum_field = create_schema_variant_enum(&existing.name, &classification.old_only_fields, &classification.new_only_fields, enums);
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

fn create_schema_variant_enum(struct_name: &str, old_fields: &[RustField], new_fields: &[RustField], enums: &mut Vec<RustEnum>) -> RustField {
    // Check if enum already exists to avoid duplicates
    let enum_name = format!("{}Variant", struct_name);
    if enums.iter().any(|e| e.name == enum_name) {
        eprintln!("🔄 Enum '{}' already exists, skipping duplicate", enum_name);
        return RustField {
            name: "schema_variant".to_string(),
            type_name: enum_name,
            is_optional: false,
            serde_rename: None,
        };
    }
    
    // Create enum variants using distinctive field names
    let mut variants = Vec::new();
    
    // For untagged enums, we need to order variants from most specific to least specific
    // New fields variant first (more fields, more specific)
    if !new_fields.is_empty() {
        let variant_name = generate_variant_name(new_fields);
        
        // Make all fields optional for variant detection
        let mut optional_new_fields = new_fields.to_vec();
        for field in &mut optional_new_fields {
            if !field.type_name.starts_with("Option<") {
                field.type_name = format!("Option<{}>", field.type_name);
                field.is_optional = true;
            }
        }
        
        variants.push(RustEnumVariant {
            name: variant_name,
            fields: optional_new_fields,
        });
    }
    
    // Old fields variant second (fewer fields, less specific)
    if !old_fields.is_empty() {
        let variant_name = generate_variant_name(old_fields);
        
        // Make all fields optional for variant detection
        let mut optional_old_fields = old_fields.to_vec();
        for field in &mut optional_old_fields {
            if !field.type_name.starts_with("Option<") {
                field.type_name = format!("Option<{}>", field.type_name);
                field.is_optional = true;
            }
        }
        
        variants.push(RustEnumVariant {
            name: variant_name,
            fields: optional_old_fields,
        });
    }
    
    // Create the enum type
    let rust_enum = RustEnum {
        name: enum_name.clone(),
        variants,
        derives: vec![
            "Debug".to_string(),
            "Clone".to_string(),
            "Serialize".to_string(),
            "Deserialize".to_string(),
        ],
    };
    
    // Add enum to the collection
    enums.push(rust_enum);
    
    // Return the field that references this enum
    RustField {
        name: "schema_variant".to_string(),
        type_name: enum_name,
        is_optional: false,
        serde_rename: None,
    }
}

fn generate_variant_name(fields: &[RustField]) -> String {
    // Find the most distinctive field name (shortest, most specific)
    let distinctive_field = fields
        .iter()
        .min_by_key(|field| field.name.len())
        .map(|field| field.name.as_str())
        .unwrap_or("Variant");
    
    // Convert to PascalCase and add "Variant" suffix
    let pascal_name = to_pascal_case(distinctive_field);
    format!("{}Variant", pascal_name)
}

fn extract_fields_from_schema(schema: &JsonSchema) -> Result<Vec<RustField>, Json2RustError> {
    match &schema.json_type {
        JsonType::Object(fields) => {
            let mut rust_fields = Vec::new();
            
            for (field_name, field_type) in fields {
                let field_type_name = match field_type {
                    JsonType::String => "String".to_string(),
                    JsonType::Number => "f64".to_string(),
                    JsonType::Boolean => "bool".to_string(),
                    JsonType::Null => "Option<serde_json::Value>".to_string(),
                    JsonType::Array(_) => "Vec<serde_json::Value>".to_string(), // Simplified for now
                    JsonType::Object(_) => "serde_json::Value".to_string(), // Simplified for now
                };
                
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
        _ => Err(Json2RustError::CodeGeneration("Schema is not an object".to_string())),
    }
}

fn create_extended_enum(_struct_name: &str, old_fields: &[RustField], new_fields: &[RustField], existing_enum: &RustEnum) -> RustEnum {
    let mut variants = existing_enum.variants.clone();
    
    // Create a new variant for the new field combination
    if !new_fields.is_empty() || !old_fields.is_empty() {
        let mut variant_fields = Vec::new();
        
        // Add old-only fields (excluding schema_variant to avoid recursion)
        for field in old_fields {
            if field.name != "schema_variant" {
                let mut optional_field = field.clone();
                if !optional_field.type_name.starts_with("Option<") {
                    optional_field.type_name = format!("Option<{}>", optional_field.type_name);
                    optional_field.is_optional = true;
                }
                variant_fields.push(optional_field);
            }
        }
        
        // Add new-only fields (excluding schema_variant to avoid recursion)
        for field in new_fields {
            if field.name != "schema_variant" {
                let mut optional_field = field.clone();
                if !optional_field.type_name.starts_with("Option<") {
                    optional_field.type_name = format!("Option<{}>", optional_field.type_name);
                    optional_field.is_optional = true;
                }
                variant_fields.push(optional_field);
            }
        }
        
        if !variant_fields.is_empty() {
            // Check if this field combination already exists in any variant
            let field_names: std::collections::HashSet<String> = variant_fields.iter().map(|f| f.name.clone()).collect();
            
            let existing_variant = variants.iter().find(|variant| {
                let existing_field_names: std::collections::HashSet<String> = variant.fields.iter().map(|f| f.name.clone()).collect();
                field_names == existing_field_names
            });
            
            if existing_variant.is_some() {
                eprintln!("🔄 Field combination already exists in variant: {:?}", existing_variant.unwrap().name);
            } else {
                // Generate variant name based on distinctive fields
                let variant_name = generate_variant_name(&variant_fields);
                
                // Additional check by name to avoid duplicates
                if !variants.iter().any(|v| v.name == variant_name) {
                    let new_variant = RustEnumVariant {
                        name: variant_name.clone(),
                        fields: variant_fields,
                    };
                    
                    variants.push(new_variant);
                    eprintln!("🆕 Added new variant to enum: {}", variant_name);
                } else {
                    eprintln!("🔄 Variant with name '{}' already exists", variant_name);
                }
            }
        } else {
            eprintln!("🔍 No fields to create variant from");
        }
    }
    
    RustEnum {
        name: existing_enum.name.clone(),
        variants,
        derives: existing_enum.derives.clone(),
    }
}

#[derive(Debug)]
struct FieldClassification {
    common_fields: Vec<RustField>,     // In both schemas - mandatory
    old_only_fields: Vec<RustField>,   // Only in old schema - mandatory
    new_only_fields: Vec<RustField>,   // Only in new schema - mandatory
}

fn classify_fields_for_extension(existing: &ExistingStruct, new_fields: &[RustField]) -> FieldClassification {
    classify_fields_for_extension_with_enums(existing, new_fields, &[])
}

fn classify_fields_for_extension_with_enums(
    existing: &ExistingStruct, 
    new_fields: &[RustField], 
    existing_enums: &[RustEnum]
) -> FieldClassification {
    let mut common_fields = Vec::new();
    let mut old_only_fields = Vec::new();
    let mut new_only_fields = Vec::new();
    
    let new_field_map: HashMap<String, &RustField> = new_fields
        .iter()
        .map(|f| (f.name.clone(), f))
        .collect();
    
    // Build a set of all fields that exist in enum variants
    let mut enum_variant_fields = std::collections::HashSet::new();
    for enum_type in existing_enums {
        for variant in &enum_type.variants {
            for field in &variant.fields {
                enum_variant_fields.insert(field.name.clone());
            }
        }
    }
    
    // Process existing struct fields (excluding schema_variant)
    for (existing_field_name, existing_field_type) in &existing.fields {
        if existing_field_name == "schema_variant" {
            continue; // Skip schema_variant field to avoid recursion
        }
        
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
    
    // Process new fields that don't exist in existing schema (excluding schema_variant)
    for new_field in new_fields {
        if new_field.name == "schema_variant" {
            eprintln!("🔍 Skipping schema_variant field: {}", new_field.name);
            continue; // Skip schema_variant field to avoid recursion
        }
        
        if existing.fields.contains_key(&new_field.name) {
            eprintln!("🔍 Field {} already exists in struct", new_field.name);
        } else if enum_variant_fields.contains(&new_field.name) {
            eprintln!("🔍 Field {} already exists in enum variant", new_field.name);
            // Don't add to new_only_fields since it already exists in enum
        } else {
            // Truly new field - doesn't exist in struct or enum variants
            eprintln!("🔍 Adding truly new field: {}", new_field.name);
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
    generate_code_with_types(&GeneratedTypes { structs: structs.to_vec(), enums: Vec::new() })
}

pub fn generate_code_with_types(types: &GeneratedTypes) -> Result<String, Json2RustError> {
    let mut code = String::new();
    
    code.push_str("use serde::{Deserialize, Serialize};\n\n");
    
    // Generate enums first
    for rust_enum in &types.enums {
        code.push_str(&generate_enum_code(rust_enum)?);
        code.push('\n');
    }
    
    // Generate structs
    for rust_struct in &types.structs {
        code.push_str(&generate_struct_code(rust_struct)?);
        code.push('\n');
    }
    
    Ok(code)
}

fn generate_enum_code(rust_enum: &RustEnum) -> Result<String, Json2RustError> {
    let mut code = String::new();
    
    let derives = rust_enum.derives.join(", ");
    code.push_str(&format!("#[derive({})]\n", derives));
    
    // Use untagged serialization for field-based variant detection
    code.push_str(&format!("#[serde(untagged)]\n"));
    code.push_str(&format!("pub enum {} {{\n", rust_enum.name));
    
    for variant in &rust_enum.variants {
        if variant.fields.is_empty() {
            code.push_str(&format!("    {},\n", variant.name));
        } else {
            code.push_str(&format!("    {} {{\n", variant.name));
            for field in &variant.fields {
                if let Some(rename) = &field.serde_rename {
                    code.push_str(&format!("        #[serde(rename = \"{}\")]\n", rename));
                }
                
                if field.is_optional {
                    code.push_str(&format!("        #[serde(skip_serializing_if = \"Option::is_none\")]\n"));
                }
                
                let field_type = if field.is_optional && !field.type_name.starts_with("Option<") {
                    format!("Option<{}>", field.type_name)
                } else {
                    field.type_name.clone()
                };
                
                code.push_str(&format!("        {}: {},\n", field.name, field_type));
            }
            code.push_str("    },\n");
        }
    }
    
    code.push_str("}\n");
    
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
    let types = GeneratedTypes { structs: structs.to_vec(), enums: Vec::new() };
    generate_code_with_types_and_preservation(&types, original_code, merge_strategy)
}

pub fn generate_code_with_types_and_preservation(
    types: &GeneratedTypes,
    original_code: Option<&str>,
    merge_strategy: &MergeStrategy,
) -> Result<String, Json2RustError> {
    if let Some(original) = original_code {
        generate_code_preserving_original(&types.structs, original, merge_strategy)
    } else {
        generate_code_with_types(types)
    }
}

pub fn generate_code_with_types_and_preservation_and_schema(
    types: &GeneratedTypes,
    original_code: Option<&str>,
    merge_strategy: &MergeStrategy,
    schema: Option<&JsonSchema>,
) -> Result<String, Json2RustError> {
    if let Some(original) = original_code {
        generate_code_preserving_original_with_schema(&types.structs, original, merge_strategy, schema)
    } else {
        generate_code_with_types(types)
    }
}

fn generate_code_preserving_original_with_schema(
    new_structs: &[RustStruct],
    original_code: &str,
    merge_strategy: &MergeStrategy,
    schema: Option<&JsonSchema>,
) -> Result<String, Json2RustError> {
    // For preservation, we need to create a mutable enum collection for potential enum generation
    let mut temp_enums = Vec::new();
    use syn::{File, Item, spanned::Spanned};
    
    let ast: File = syn::parse_str(original_code)
        .map_err(|e| Json2RustError::RustParsing(format!("Failed to parse original code: {}", e)))?;
    
    // First, extract existing enums from the original code
    let mut existing_enum_names = std::collections::HashSet::new();
    for item in &ast.items {
        if let Item::Enum(item_enum) = item {
            let rust_enum = parse_enum_from_item(item_enum)?;
            existing_enum_names.insert(rust_enum.name.clone());
            temp_enums.push(rust_enum);
            eprintln!("🔍 Found existing enum: {}", item_enum.ident);
        }
    }
    
    // Create a map of new structs by name for quick lookup
    let new_struct_map: std::collections::HashMap<String, &RustStruct> = new_structs
        .iter()
        .map(|s| (s.name.clone(), s))
        .collect();
    
    let mut result = String::new();
    let mut last_end = 0;
    
    // Find struct spans and sort them by position
    let mut struct_replacements = Vec::new();
    let mut enum_replacements = Vec::new();
    
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
                
                // Generate the correct field list from the original schema
                let fields_to_use = if let Some(schema) = schema {
                    if schema.name == struct_name {
                        // Extract fields from the schema
                        extract_fields_from_schema(schema)?
                    } else {
                        new_struct.fields.clone()
                    }
                } else {
                    new_struct.fields.clone()
                };
                
                // Extend the existing struct with new fields from JSON
                let initial_enum_count = temp_enums.len();
                eprintln!("🔍 using fields: {:?}", fields_to_use);
                let extended_struct = extend_existing_struct(&existing_struct, fields_to_use, &mut temp_enums, merge_strategy);
                
                // Check if any enums were modified
                if temp_enums.len() != initial_enum_count {
                    eprintln!("🔄 Enum count changed from {} to {}", initial_enum_count, temp_enums.len());
                }
                
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
    
    // After processing all structs, check for enum replacements
    for item in &ast.items {
        if let Item::Enum(item_enum) = item {
            let enum_name = item_enum.ident.to_string();
            // Check if this enum was modified
            if let Some(modified_enum) = temp_enums.iter().find(|e| e.name == enum_name) {
                // Check if the enum was actually modified (has different content)
                let original_enum = parse_enum_from_item(item_enum)?;
                if enum_was_modified(&original_enum, modified_enum) {
                    let start_byte = find_enum_start(original_code, &enum_name)?;
                    let end_byte = find_enum_end(original_code, start_byte)?;
                    
                    enum_replacements.push(EnumReplacement {
                        start: start_byte,
                        end: end_byte,
                        new_enum: modified_enum.clone(),
                        name: enum_name.clone(),
                    });
                    
                    eprintln!("🔄 Will replace enum '{}' with modified version", enum_name);
                }
            }
        }
    }
    
    // Sort replacements by start position
    struct_replacements.sort_by_key(|r| r.start);
    enum_replacements.sort_by_key(|r| r.start);
    
    // Combine and sort all replacements by position
    let mut all_replacements: Vec<(usize, usize, String)> = Vec::new();
    
    // Add struct replacements
    for replacement in struct_replacements {
        let code = generate_struct_code(&replacement.new_struct)?;
        all_replacements.push((replacement.start, replacement.end, code));
    }
    
    // Add enum replacements
    for replacement in enum_replacements {
        let code = generate_enum_code(&replacement.new_enum)?;
        all_replacements.push((replacement.start, replacement.end, code));
    }
    
    // Sort all replacements by start position
    all_replacements.sort_by_key(|r| r.0);
    
    // Process the file, preserving original text and replacing specific items
    for (start, end, code) in all_replacements {
        // Add original text up to this item
        result.push_str(&original_code[last_end..start]);
        
        // Add the new item code
        result.push_str(&code);
        
        last_end = end;
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
    
    // Add any generated enums (only new ones, not existing ones)
    for rust_enum in &temp_enums {
        if !existing_enum_names.contains(&rust_enum.name) {
            result.push('\n');
            result.push_str(&generate_enum_code(rust_enum)?);
            eprintln!("✨ Added new enum '{}'", rust_enum.name);
        }
    }
    
    Ok(result)
}

fn generate_code_preserving_original(
    new_structs: &[RustStruct],
    original_code: &str,
    merge_strategy: &MergeStrategy,
) -> Result<String, Json2RustError> {
    // For preservation, we need to create a mutable enum collection for potential enum generation
    let mut temp_enums = Vec::new();
    use syn::{File, Item, spanned::Spanned};
    
    let ast: File = syn::parse_str(original_code)
        .map_err(|e| Json2RustError::RustParsing(format!("Failed to parse original code: {}", e)))?;
    
    // First, extract existing enums from the original code
    let mut existing_enum_names = std::collections::HashSet::new();
    for item in &ast.items {
        if let Item::Enum(item_enum) = item {
            let rust_enum = parse_enum_from_item(item_enum)?;
            existing_enum_names.insert(rust_enum.name.clone());
            temp_enums.push(rust_enum);
            eprintln!("🔍 Found existing enum: {}", item_enum.ident);
        }
    }
    
    // Create a map of new structs by name for quick lookup
    let new_struct_map: std::collections::HashMap<String, &RustStruct> = new_structs
        .iter()
        .map(|s| (s.name.clone(), s))
        .collect();
    
    let mut result = String::new();
    let mut last_end = 0;
    
    // Find struct spans and sort them by position
    let mut struct_replacements = Vec::new();
    let mut enum_replacements = Vec::new();
    
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
                let initial_enum_count = temp_enums.len();
                eprintln!("🔍 new_struct.fields: {:?}", new_struct.fields);
                let extended_struct = extend_existing_struct(&existing_struct, new_struct.fields.clone(), &mut temp_enums, merge_strategy);
                
                // Check if any enums were modified
                if temp_enums.len() != initial_enum_count {
                    eprintln!("🔄 Enum count changed from {} to {}", initial_enum_count, temp_enums.len());
                }
                
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
    
    // After processing all structs, check for enum replacements
    for item in &ast.items {
        if let Item::Enum(item_enum) = item {
            let enum_name = item_enum.ident.to_string();
            // Check if this enum was modified
            if let Some(modified_enum) = temp_enums.iter().find(|e| e.name == enum_name) {
                // Check if the enum was actually modified (has different content)
                let original_enum = parse_enum_from_item(item_enum)?;
                if enum_was_modified(&original_enum, modified_enum) {
                    let start_byte = find_enum_start(original_code, &enum_name)?;
                    let end_byte = find_enum_end(original_code, start_byte)?;
                    
                    enum_replacements.push(EnumReplacement {
                        start: start_byte,
                        end: end_byte,
                        new_enum: modified_enum.clone(),
                        name: enum_name.clone(),
                    });
                    
                    eprintln!("🔄 Will replace enum '{}' with modified version", enum_name);
                }
            }
        }
    }
    
    // Sort replacements by start position
    struct_replacements.sort_by_key(|r| r.start);
    enum_replacements.sort_by_key(|r| r.start);
    
    // Combine and sort all replacements by position
    let mut all_replacements: Vec<(usize, usize, String)> = Vec::new();
    
    // Add struct replacements
    for replacement in struct_replacements {
        let code = generate_struct_code(&replacement.new_struct)?;
        all_replacements.push((replacement.start, replacement.end, code));
    }
    
    // Add enum replacements
    for replacement in enum_replacements {
        let code = generate_enum_code(&replacement.new_enum)?;
        all_replacements.push((replacement.start, replacement.end, code));
    }
    
    // Sort all replacements by start position
    all_replacements.sort_by_key(|r| r.0);
    
    // Process the file, preserving original text and replacing specific items
    for (start, end, code) in all_replacements {
        // Add original text up to this item
        result.push_str(&original_code[last_end..start]);
        
        // Add the new item code
        result.push_str(&code);
        
        last_end = end;
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
    
    // Add any generated enums (only new ones, not existing ones)
    for rust_enum in &temp_enums {
        if !existing_enum_names.contains(&rust_enum.name) {
            result.push('\n');
            result.push_str(&generate_enum_code(rust_enum)?);
            eprintln!("✨ Added new enum '{}'", rust_enum.name);
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

struct EnumReplacement {
    start: usize,
    end: usize,
    new_enum: RustEnum,
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

fn enum_was_modified(original: &RustEnum, modified: &RustEnum) -> bool {
    // Simple comparison - if variant count differs, it was modified
    if original.variants.len() != modified.variants.len() {
        return true;
    }
    
    // Check if variant names differ
    for (orig_variant, mod_variant) in original.variants.iter().zip(modified.variants.iter()) {
        if orig_variant.name != mod_variant.name {
            return true;
        }
        if orig_variant.fields.len() != mod_variant.fields.len() {
            return true;
        }
    }
    
    false
}

fn find_enum_start(source: &str, enum_name: &str) -> Result<usize, Json2RustError> {
    let lines: Vec<&str> = source.lines().collect();
    let mut enum_line_idx = None;
    
    // Find the line with the enum definition
    for (i, line) in lines.iter().enumerate() {
        if line.trim().contains(&format!("enum {}", enum_name)) {
            enum_line_idx = Some(i);
            break;
        }
    }
    
    if let Some(enum_idx) = enum_line_idx {
        // Look backwards for derive attributes
        let mut start_idx = enum_idx;
        
        while start_idx > 0 {
            let prev_line = lines[start_idx - 1].trim();
            if prev_line.starts_with("#[derive(") || prev_line.starts_with("#[serde(") || prev_line.starts_with("pub ") || prev_line.starts_with("//") || prev_line.is_empty() {
                start_idx -= 1;
            } else {
                break;
            }
        }
        
        // Calculate byte position
        let byte_pos = lines[..start_idx].iter().map(|l| l.len() + 1).sum::<usize>();
        Ok(byte_pos)
    } else {
        Err(Json2RustError::CodeGeneration(format!("Could not find enum {} in source", enum_name)))
    }
}

fn find_enum_end(source: &str, start: usize) -> Result<usize, Json2RustError> {
    let remaining = &source[start..];
    let mut brace_count = 0;
    let mut in_enum = false;
    
    for (i, ch) in remaining.char_indices() {
        match ch {
            '{' => {
                brace_count += 1;
                in_enum = true;
            }
            '}' => {
                brace_count -= 1;
                if in_enum && brace_count == 0 {
                    return Ok(start + i + 1);
                }
            }
            _ => {}
        }
    }
    
    Err(Json2RustError::CodeGeneration("Could not find end of enum".to_string()))
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

fn parse_enum_from_item(item_enum: &syn::ItemEnum) -> Result<RustEnum, Json2RustError> {
    let mut variants = Vec::new();
    
    for variant in &item_enum.variants {
        let variant_name = variant.ident.to_string();
        let mut fields = Vec::new();
        
        // Parse variant fields
        if let syn::Fields::Named(named_fields) = &variant.fields {
            for field in &named_fields.named {
                if let Some(field_name) = &field.ident {
                    let field_type = type_to_string(&field.ty);
                    
                    // Check if field is optional based on type
                    let is_optional = field_type.starts_with("Option<");
                    
                    // Extract serde rename attribute if present
                    let serde_rename = extract_serde_rename(&field.attrs);
                    
                    fields.push(RustField {
                        name: field_name.to_string(),
                        type_name: field_type,
                        is_optional,
                        serde_rename,
                    });
                }
            }
        }
        
        variants.push(RustEnumVariant {
            name: variant_name,
            fields,
        });
    }
    
    // Extract derive attributes
    let derives = extract_derives(&item_enum.attrs);
    
    Ok(RustEnum {
        name: item_enum.ident.to_string(),
        variants,
        derives,
    })
}

fn extract_serde_rename(_attrs: &[syn::Attribute]) -> Option<String> {
    // For now, return None to avoid complex attribute parsing
    // TODO: Implement proper serde rename extraction
    None
}

fn extract_derives(_attrs: &[syn::Attribute]) -> Vec<String> {
    // For now, return default derives to avoid complex attribute parsing
    // TODO: Implement proper derive extraction
    vec![
        "Debug".to_string(),
        "Clone".to_string(),
        "Serialize".to_string(),
        "Deserialize".to_string(),
    ]
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