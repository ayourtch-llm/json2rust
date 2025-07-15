struct User {
    nxame: String,
    age: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Profile {
    bio: String,
    avatar_url: Option<String>,
}
