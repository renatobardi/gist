use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    pub id: String,
    /// Normalized: lowercase + trimmed. Used as the deduplication key.
    pub name: String,
    /// Original casing from Gemini response.
    pub display_name: String,
    pub description: String,
    pub domain: String,
    pub created_at: String,
}

/// Normalize a concept name for deduplication: lowercase + trim.
pub fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
}

/// A concept as returned by the Gemini structured response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiConcept {
    pub name: String,
    pub description: String,
    pub domain: String,
    /// Weight of this concept within the insight. Defaults to 0.5 if omitted by Gemini.
    pub relevance_weight: Option<f64>,
    pub related_concepts: Vec<GeminiRelatedConcept>,
}

/// A related concept entry within a `GeminiConcept`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiRelatedConcept {
    pub name: String,
    /// e.g. "enables", "contrasts_with", "is_part_of", "extends", "related". Defaults to "related".
    pub relation_type: Option<String>,
    /// 0.0–1.0. Defaults to 0.5 if omitted.
    pub strength: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_name_lowercases_and_trims() {
        assert_eq!(normalize_name("  Clean Code  "), "clean code");
    }

    #[test]
    fn normalize_name_already_normalized_is_idempotent() {
        assert_eq!(normalize_name("clean code"), "clean code");
    }

    #[test]
    fn normalize_name_handles_mixed_case() {
        assert_eq!(normalize_name("SOLID Principles"), "solid principles");
    }
}
