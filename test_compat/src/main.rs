use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub age: i32,
    pub nxame: String,
    pub name: String,
    pub active: bool,
    pub premium: bool,
    pub email: String,
    pub last_login: String,
}

fn main() {
    println!("🧪 Testing backward compatibility of extended structs...");
    
    // Test 1: Original JSON (should work)
    let original_json = r#"{"nxame": "Old User", "age": 25}"#;
    match serde_json::from_str::<User>(original_json) {
        Ok(user) => println!("✅ Original JSON works: {:?}", user),
        Err(e) => println!("❌ Original JSON failed: {}", e),
    }
    
    // Test 2: New JSON (should work)
    let new_json = r#"{"name": "John Doe", "age": 30, "email": "john@example.com", "active": true, "premium": true, "last_login": "2023-01-01"}"#;
    match serde_json::from_str::<User>(new_json) {
        Ok(user) => println!("✅ New JSON works: {:?}", user),
        Err(e) => println!("❌ New JSON failed: {}", e),
    }
    
    // Test 3: Mixed JSON (should work)
    let mixed_json = r#"{"nxame": "Mixed User", "age": 35, "email": "mixed@example.com"}"#;
    match serde_json::from_str::<User>(mixed_json) {
        Ok(user) => println!("✅ Mixed JSON works: {:?}", user),
        Err(e) => println!("❌ Mixed JSON failed: {}", e),
    }
    
    // Test 4: Minimal JSON (should work)
    let minimal_json = r#"{"age": 40}"#;
    match serde_json::from_str::<User>(minimal_json) {
        Ok(user) => println!("✅ Minimal JSON works: {:?}", user),
        Err(e) => println!("❌ Minimal JSON failed: {}", e),
    }
    
    println!("🎉 Backward compatibility tests completed!");
}
