#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub age: i32,
    pub schema_variant: UserVariant,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
struct Profile {
    bio: String,
    avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UserVariant {
    NameVariant {
        #[serde(skip_serializing_if = "Option::is_none")]
        active: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    NxameVariant {
        #[serde(skip_serializing_if = "Option::is_none")]
        nxame: Option<String>,
    },
}
