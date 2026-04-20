use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub domain: String,
}

impl Concept {
    pub fn normalize_name(name: &str) -> String {
        name.trim().to_lowercase()
    }
}
