use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    pub id: String,
    /// Normalized lowercase name (used as the unique key).
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptRelation {
    pub from_concept_name: String,
    pub to_concept_name: String,
    pub relation_type: String,
    pub strength: f64,
}

/// Normalize a concept name to lowercase, trimmed.
pub fn normalize_concept_name(display_name: &str) -> String {
    display_name.trim().to_lowercase()
}
