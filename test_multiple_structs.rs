use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    name: String,
    age: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Product {
    id: u32,
    title: String,
    price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Order {
    order_id: String,
    customer_name: String,
    items: Vec<String>,
}