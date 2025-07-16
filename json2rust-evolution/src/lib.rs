//! # JSON to Rust Evolution Library
//! 
//! This library provides advanced API evolution capabilities by analyzing existing Rust code 
//! and JSON data structures to generate optimized type definitions.
//! 
//! ## Key Features
//! 
//! - **Shape Expansion**: Every optional field expands into two shapes (with/without), enums expand into each variant
//! - **Common Field Extraction**: Fields present in all shapes become mandatory
//! - **Variant Optimization**: Find overlapping field sets and merge similar shapes
//! - **Recursive Application**: Apply algorithm recursively to nested structures
//! 
//! ## Example
//! 
//! ```rust
//! use json2rust_evolution::evolve_rust_types;
//! 
//! let existing_rust = r#"
//!     struct User {
//!         name: String,
//!         age: i32,
//!     }
//! "#;
//! 
//! let json_data = r#"{"name": "John", "age": 30, "email": "john@example.com"}"#;
//! 
//! let evolved_rust = evolve_rust_types(existing_rust, json_data, "User", false).unwrap();
//! println!("{}", evolved_rust);
//! ```

pub mod parser;
pub mod shape;
pub mod evolution;
pub mod optimizer;
pub mod generator;
pub mod surgery;

use crate::parser::RustParser;
use crate::evolution::ApiEvolution;
use anyhow::Result;
use std::collections::HashMap;

/// Evolve Rust types based on existing code and JSON data
/// 
/// # Arguments
/// 
/// * `existing_rust_code` - The existing Rust code containing type definitions
/// * `json_data` - JSON string containing the data structure to analyze
/// * `type_name` - The name of the target type to evolve
/// * `verbose` - Whether to enable verbose output
/// 
/// # Returns
/// 
/// A `Result` containing the evolved Rust code as a `String`, or an error if the evolution fails.
/// 
/// # Example
/// 
/// ```rust
/// use json2rust_evolution::evolve_rust_types;
/// 
/// let existing_rust = r#"
///     struct User {
///         name: String,
///         age: i32,
///     }
/// "#;
/// 
/// let json_data = r#"{"name": "John", "age": 30, "email": "john@example.com"}"#;
/// 
/// match evolve_rust_types(existing_rust, json_data, "User", false) {
///     Ok(evolved_code) => println!("Evolved code:\n{}", evolved_code),
///     Err(e) => eprintln!("Evolution failed: {}", e),
/// }
/// ```
pub fn evolve_rust_types(
    existing_rust_code: &str,
    json_data: &str,
    type_name: &str,
    verbose: bool,
) -> Result<String> {
    // Parse the JSON data
    let json_value: serde_json::Value = serde_json::from_str(json_data)?;
    
    // Parse existing Rust types
    let existing_types = if existing_rust_code.trim().is_empty() {
        HashMap::new()
    } else {
        let mut parser = RustParser::new();
        parser.parse_types(existing_rust_code)?
    };
    
    // Create evolution engine
    let mut evolution = ApiEvolution::new(existing_types, verbose);
    
    // Perform evolution
    let result = evolution.evolve_with_json(&json_value, type_name)?;
    
    // Generate the evolved Rust code
    let evolved_code = result.generate_rust_code()?;
    
    // If there was existing code, append it after the evolved types
    if !existing_rust_code.trim().is_empty() {
        // Extract any types that weren't evolved
        let mut parser = RustParser::new();
        let original_types = parser.parse_types(existing_rust_code)?;
        
        let mut additional_code = String::new();
        for (type_name_key, type_info) in &original_types {
            // Check if this type was evolved (would be mentioned in the evolved code)
            if !evolved_code.contains(&format!("struct {}", type_name_key)) &&
               !evolved_code.contains(&format!("enum {}", type_name_key)) &&
               type_name_key != type_name {
                // Add the original type definition
                if let Some((start, end)) = parser.find_type_span(type_name_key, matches!(type_info.kind, crate::parser::TypeKind::Struct { .. })) {
                    if start < existing_rust_code.len() && end <= existing_rust_code.len() {
                        let type_def = &existing_rust_code[start..end];
                        if !additional_code.contains(type_def) {
                            additional_code.push_str(type_def);
                            additional_code.push('\n');
                        }
                    }
                }
            }
        }
        
        if !additional_code.is_empty() {
            Ok(format!("{}\n{}", evolved_code, additional_code))
        } else {
            Ok(evolved_code)
        }
    } else {
        Ok(evolved_code)
    }
}

/// Evolve Rust types with custom options
/// 
/// This is a more advanced version of `evolve_rust_types` that allows for additional configuration.
/// 
/// # Arguments
/// 
/// * `existing_rust_code` - The existing Rust code containing type definitions
/// * `json_data` - JSON string containing the data structure to analyze
/// * `type_name` - The name of the target type to evolve
/// * `options` - Evolution options
/// 
/// # Example
/// 
/// ```rust
/// use json2rust_evolution::{evolve_rust_types_with_options, EvolutionOptions};
/// 
/// let options = EvolutionOptions {
///     verbose: true,
///     preserve_original_types: true,
/// };
/// 
/// let result = evolve_rust_types_with_options(
///     "struct User { name: String }",
///     r#"{"name": "John", "email": "john@example.com"}"#,
///     "User",
///     options
/// );
/// ```
pub fn evolve_rust_types_with_options(
    existing_rust_code: &str,
    json_data: &str,
    type_name: &str,
    options: EvolutionOptions,
) -> Result<String> {
    // For now, just use the basic function with verbose option
    evolve_rust_types(existing_rust_code, json_data, type_name, options.verbose)
}

/// Options for controlling the evolution process
#[derive(Debug, Clone)]
pub struct EvolutionOptions {
    /// Whether to enable verbose output during evolution
    pub verbose: bool,
    /// Whether to preserve original type definitions that weren't evolved
    pub preserve_original_types: bool,
}

impl Default for EvolutionOptions {
    fn default() -> Self {
        Self {
            verbose: false,
            preserve_original_types: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_evolution() {
        let existing_rust = r#"
            struct User {
                name: String,
                age: i32,
            }
        "#;

        let json_data = r#"{"name": "John", "age": 30, "email": "john@example.com"}"#;

        let result = evolve_rust_types(existing_rust, json_data, "User", false);
        assert!(result.is_ok());
        
        let evolved_code = result.unwrap();
        assert!(evolved_code.contains("struct User"));
        assert!(evolved_code.contains("email"));
    }

    #[test]
    fn test_empty_existing_code() {
        let json_data = r#"{"name": "John", "age": 30}"#;

        let result = evolve_rust_types("", json_data, "User", false);
        assert!(result.is_ok());
        
        let evolved_code = result.unwrap();
        assert!(evolved_code.contains("struct User"));
        assert!(evolved_code.contains("name"));
        assert!(evolved_code.contains("age"));
    }

    #[test]
    fn test_invalid_json() {
        let existing_rust = "struct User { name: String }";
        let invalid_json = r#"{"name": "John", "age":}"#;

        let result = evolve_rust_types(existing_rust, invalid_json, "User", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_with_options() {
        let options = EvolutionOptions {
            verbose: false,
            preserve_original_types: true,
        };

        let result = evolve_rust_types_with_options(
            "struct User { name: String }",
            r#"{"name": "John", "email": "john@example.com"}"#,
            "User",
            options,
        );

        assert!(result.is_ok());
    }
}
