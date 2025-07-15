use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum User {
    // Legacy first - if nxame is present, it's definitely legacy
    Legacy {
        age: i32,
        nxame: String,  // Required field - if present, it's legacy
    },
    // Current second - fallback for new schema
    Current {
        age: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        active: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_login: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        premium: Option<bool>,
    },
}


fn main() {
    println!("🧪 Testing enum-based schema variant deserialization...");
    
    // Test 1: New JSON (should match Current variant)
    let new_json = r#"{"age": 30, "name": "John Doe", "email": "john@example.com", "active": true, "premium": true, "last_login": "2023-01-01"}"#;
    match serde_json::from_str::<User>(new_json) {
        Ok(user) => {
            println!("✅ New JSON deserialized successfully:");
            match user {
                User::Current { age, name, email, active, premium, last_login } => {
                    println!("   Age: {}", age);
                    println!("   Variant: Current");
                    println!("   Name: {:?}", name);
                    println!("   Email: {:?}", email);
                    println!("   Active: {:?}", active);
                    println!("   Premium: {:?}", premium);
                    println!("   Last Login: {:?}", last_login);
                }
                User::Legacy { age, nxame } => {
                    println!("   Age: {}", age);
                    println!("   Variant: Legacy");
                    println!("   Nxame: {}", nxame);
                }
            }
        }
        Err(e) => println!("❌ New JSON failed: {}", e),
    }
    
    // Test 2: Old JSON (should match Legacy variant)
    let old_json = r#"{"age": 25, "nxame": "Old User"}"#;
    match serde_json::from_str::<User>(old_json) {
        Ok(user) => {
            println!("✅ Old JSON deserialized successfully:");
            match user {
                User::Current { age, name, email, active, premium, last_login } => {
                    println!("   Age: {}", age);
                    println!("   Variant: Current");
                    println!("   Name: {:?}", name);
                    println!("   Email: {:?}", email);
                    println!("   Active: {:?}", active);
                    println!("   Premium: {:?}", premium);
                    println!("   Last Login: {:?}", last_login);
                }
                User::Legacy { age, nxame } => {
                    println!("   Age: {}", age);
                    println!("   Variant: Legacy");
                    println!("   Nxame: {}", nxame);
                }
            }
        }
        Err(e) => println!("❌ Old JSON failed: {}", e),
    }
    
    // Test 3: Mixed JSON (should match Current variant - more fields)
    let mixed_json = r#"{"age": 35, "nxame": "Mixed User", "email": "mixed@example.com"}"#;
    match serde_json::from_str::<User>(mixed_json) {
        Ok(user) => {
            println!("✅ Mixed JSON deserialized successfully:");
            match user {
                User::Current { age, name, email, active, premium, last_login } => {
                    println!("   Age: {}", age);
                    println!("   Variant: Current");
                    println!("   Name: {:?}", name);
                    println!("   Email: {:?}", email);
                    println!("   Active: {:?}", active);
                    println!("   Premium: {:?}", premium);
                    println!("   Last Login: {:?}", last_login);
                }
                User::Legacy { age, nxame } => {
                    println!("   Age: {}", age);
                    println!("   Variant: Legacy");
                    println!("   Nxame: {}", nxame);
                }
            }
        }
        Err(e) => println!("❌ Mixed JSON failed: {}", e),
    }
    
    // Test 4: Minimal JSON (should match Legacy variant)
    let minimal_json = r#"{"age": 40}"#;
    match serde_json::from_str::<User>(minimal_json) {
        Ok(user) => {
            println!("✅ Minimal JSON deserialized successfully:");
            match user {
                User::Current { age, name, email, active, premium, last_login } => {
                    println!("   Age: {}", age);
                    println!("   Variant: Current");
                    println!("   Name: {:?}", name);
                    println!("   Email: {:?}", email);
                    println!("   Active: {:?}", active);
                    println!("   Premium: {:?}", premium);
                    println!("   Last Login: {:?}", last_login);
                }
                User::Legacy { age, nxame } => {
                    println!("   Age: {}", age);
                    println!("   Variant: Legacy");
                    println!("   Nxame: {}", nxame);
                }
            }
        }
        Err(e) => println!("❌ Minimal JSON failed: {}", e),
    }
    
    println!("🎉 Enum deserialization tests completed!");
}