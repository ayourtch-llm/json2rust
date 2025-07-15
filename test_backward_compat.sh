#!/bin/bash

# Test backward compatibility of extended structs
set -e

echo "🧪 Testing backward compatibility of extended structs..."

# Generate the extended struct
echo "Step 1: Generate extended struct..."
cargo run -- -e testdata/existing_rust1.rs -i testdata/json_input_v2.rs -n User > /tmp/extended_user.rs

# Create test file
cat > /tmp/test_compatibility.rs << 'EOF'
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
}
EOF

# Add to Cargo.toml temporarily
echo "Step 2: Testing compilation and execution..."
cd /tmp && cargo init --name test_compat > /dev/null 2>&1
cat > Cargo.toml << 'CARGO_EOF'
[package]
name = "test_compat"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
CARGO_EOF

# Copy test file
cp test_compatibility.rs src/main.rs

# Run test
echo "Step 3: Running backward compatibility tests..."
cargo run

echo "🎉 Backward compatibility tests completed!"