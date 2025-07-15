use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub age: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nxame: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_login: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub premium: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
}

fn main() {
    // Test 1: Original JSON (should work)
    let original_json = r#"{"nxame": "Old User", "age": 25}"#;
    let user1: Result<User, _> = serde_json::from_str(original_json);
    println!("Original JSON deserialization: {:?}", user1);
    
    // Test 2: New JSON (should work)
    let new_json = r#"{"name": "John Doe", "age": 30, "email": "john@example.com", "active": true, "premium": true, "last_login": "2023-01-01"}"#;
    let user2: Result<User, _> = serde_json::from_str(new_json);
    println!("New JSON deserialization: {:?}", user2);
    
    // Test 3: Mixed JSON (should work)
    let mixed_json = r#"{"nxame": "Mixed User", "age": 35, "email": "mixed@example.com"}"#;
    let user3: Result<User, _> = serde_json::from_str(mixed_json);
    println!("Mixed JSON deserialization: {:?}", user3);
}