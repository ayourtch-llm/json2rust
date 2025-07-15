#[cfg(test)]
mod existing_struct_tests {
    use crate::*;
    
    #[test]
    fn test_extend_struct_with_new_fields() {
        let existing_code = r#"
            struct User {
                name: String,
                age: i32,
            }
        "#;
        
        let json = r#"{"name": "John", "age": 30, "email": "john@example.com"}"#;
        
        let existing_structs = parse_existing_structs(existing_code).unwrap();
        let schema = analyze_json(json, "User").unwrap();
        let rust_structs = generate_rust_structs(&schema, &existing_structs).unwrap();
        
        assert_eq!(rust_structs.len(), 1);
        let user_struct = &rust_structs[0];
        assert_eq!(user_struct.name, "User");
        assert_eq!(user_struct.fields.len(), 3);
        
        // Check that existing fields preserve their types
        let name_field = user_struct.fields.iter().find(|f| f.name == "name").unwrap();
        assert_eq!(name_field.type_name, "String");
        
        let age_field = user_struct.fields.iter().find(|f| f.name == "age").unwrap();
        assert_eq!(age_field.type_name, "i32"); // Type preserved
        
        let email_field = user_struct.fields.iter().find(|f| f.name == "email").unwrap();
        assert_eq!(email_field.type_name, "String");
    }
    
    #[test]
    fn test_extend_struct_with_missing_fields() {
        let existing_code = r#"
            struct User {
                name: String,
                age: i32,
                email: String,
            }
        "#;
        
        let json = r#"{"name": "John", "age": 30}"#;
        
        let existing_structs = parse_existing_structs(existing_code).unwrap();
        let schema = analyze_json(json, "User").unwrap();
        let rust_structs = generate_rust_structs(&schema, &existing_structs).unwrap();
        
        assert_eq!(rust_structs.len(), 1);
        let user_struct = &rust_structs[0];
        assert_eq!(user_struct.name, "User");
        assert_eq!(user_struct.fields.len(), 3);
        
        // Check that missing field becomes optional
        let email_field = user_struct.fields.iter().find(|f| f.name == "email").unwrap();
        assert_eq!(email_field.type_name, "Option<String>");
        assert!(email_field.is_optional);
    }
    
    #[test]
    fn test_numeric_type_preservation() {
        let existing_code = r#"
            struct Stats {
                count: u32,
                average: f32,
                total: i64,
            }
        "#;
        
        let json = r#"{"count": 10, "average": 5.5, "total": 1000}"#;
        
        let existing_structs = parse_existing_structs(existing_code).unwrap();
        let schema = analyze_json(json, "Stats").unwrap();
        let rust_structs = generate_rust_structs(&schema, &existing_structs).unwrap();
        
        assert_eq!(rust_structs.len(), 1);
        let stats_struct = &rust_structs[0];
        
        // Check that numeric types are preserved
        let count_field = stats_struct.fields.iter().find(|f| f.name == "count").unwrap();
        assert_eq!(count_field.type_name, "u32");
        
        let average_field = stats_struct.fields.iter().find(|f| f.name == "average").unwrap();
        assert_eq!(average_field.type_name, "f32");
        
        let total_field = stats_struct.fields.iter().find(|f| f.name == "total").unwrap();
        assert_eq!(total_field.type_name, "i64");
    }
    
    #[test]
    fn test_option_type_handling() {
        let existing_code = r#"
            struct Profile {
                name: String,
                bio: Option<String>,
                age: Option<i32>,
            }
        "#;
        
        let json = r#"{"name": "Alice", "bio": "Developer", "location": "NYC"}"#;
        
        let existing_structs = parse_existing_structs(existing_code).unwrap();
        let schema = analyze_json(json, "Profile").unwrap();
        let rust_structs = generate_rust_structs(&schema, &existing_structs).unwrap();
        
        assert_eq!(rust_structs.len(), 1);
        let profile_struct = &rust_structs[0];
        
        // Check that existing Option types are preserved
        let bio_field = profile_struct.fields.iter().find(|f| f.name == "bio").unwrap();
        assert_eq!(bio_field.type_name, "Option<String>"); // Should remain Option
        
        let age_field = profile_struct.fields.iter().find(|f| f.name == "age").unwrap();
        assert_eq!(age_field.type_name, "Option<i32>"); // Missing field stays Option
        
        let location_field = profile_struct.fields.iter().find(|f| f.name == "location").unwrap();
        assert_eq!(location_field.type_name, "String"); // New field is not optional
    }
    
    #[test]
    fn test_low_similarity_creates_new_struct() {
        let existing_code = r#"
            struct User {
                name: String,
                age: i32,
            }
        "#;
        
        let json = r#"{"product_id": 123, "price": 29.99, "category": "electronics"}"#;
        
        let existing_structs = parse_existing_structs(existing_code).unwrap();
        let schema = analyze_json(json, "Product").unwrap();
        let rust_structs = generate_rust_structs(&schema, &existing_structs).unwrap();
        
        assert_eq!(rust_structs.len(), 1);
        let product_struct = &rust_structs[0];
        assert_eq!(product_struct.name, "Product"); // New struct created
        assert_eq!(product_struct.fields.len(), 3);
    }
    
    #[test]
    fn test_array_with_existing_item_struct() {
        let existing_code = r#"
            struct Item {
                id: u32,
                name: String,
            }
        "#;
        
        let json = r#"[{"id": 1, "name": "Apple", "price": 1.99}, {"id": 2, "name": "Banana", "price": 0.59}]"#;
        
        let existing_structs = parse_existing_structs(existing_code).unwrap();
        let schema = analyze_json(json, "Items").unwrap();
        let rust_structs = generate_rust_structs(&schema, &existing_structs).unwrap();
        
        assert_eq!(rust_structs.len(), 2);
        
        // Check that item struct is extended
        let item_struct = rust_structs.iter().find(|s| s.name == "Item").unwrap();
        assert_eq!(item_struct.fields.len(), 3);
        
        // Check that existing fields preserve their types
        let id_field = item_struct.fields.iter().find(|f| f.name == "id").unwrap();
        assert_eq!(id_field.type_name, "u32"); // Type preserved
        
        let name_field = item_struct.fields.iter().find(|f| f.name == "name").unwrap();
        assert_eq!(name_field.type_name, "String");
        
        let price_field = item_struct.fields.iter().find(|f| f.name == "price").unwrap();
        assert_eq!(price_field.type_name, "f64");
    }
}