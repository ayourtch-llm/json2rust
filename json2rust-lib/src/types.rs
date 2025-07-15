use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MergeStrategy {
    Optional,  // Make conflicting fields optional (default)
    Enum,      // Generate enums for incompatible field groups
    Hybrid,    // Use enums for field groups, optionals for individual fields
}

impl From<&str> for MergeStrategy {
    fn from(s: &str) -> Self {
        match s {
            "optional" => MergeStrategy::Optional,
            "enum" => MergeStrategy::Enum,
            "hybrid" => MergeStrategy::Hybrid,
            _ => MergeStrategy::Optional,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RustStruct {
    pub name: String,
    pub fields: Vec<RustField>,
    pub derives: Vec<String>,
    pub is_optional: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RustEnum {
    pub name: String,
    pub variants: Vec<RustEnumVariant>,
    pub derives: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RustEnumVariant {
    pub name: String,
    pub fields: Vec<RustField>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RustType {
    Struct(RustStruct),
    Enum(RustEnum),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RustField {
    pub name: String,
    pub type_name: String,
    pub is_optional: bool,
    pub serde_rename: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsonType {
    String,
    Number,
    Boolean,
    Array(Box<JsonType>),
    Object(HashMap<String, JsonType>),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JsonSchema {
    pub name: String,
    pub json_type: JsonType,
    pub optional: bool,
}

#[derive(Debug, Clone)]
pub struct ExistingStruct {
    pub name: String,
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Error)]
pub enum Json2RustError {
    #[error("JSON parsing error: {0}")]
    JsonParsing(#[from] serde_json::Error),
    
    #[error("Rust parsing error: {0}")]
    RustParsing(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Code generation error: {0}")]
    CodeGeneration(String),
}