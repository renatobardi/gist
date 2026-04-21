use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptWithWeight {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub domain: String,
    pub relevance_weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightDetail {
    pub id: String,
    pub summary: String,
    pub key_points: Vec<String>,
    pub concepts: Vec<ConceptWithWeight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insight {
    pub id: String,
    pub summary: String,
    pub key_points: Vec<String>,
    pub raw_gemini_response: String,
}

/// The structured response that Gemini returns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiResponse {
    pub summary: String,
    pub key_points: Vec<String>,
    pub concepts: Vec<ExtractedConcept>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedConcept {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub domain: String,
    pub relevance_weight: f64,
    #[serde(default)]
    pub related_concepts: Vec<RelatedConceptRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedConceptRef {
    pub name: String,
    #[serde(default = "default_relation_type")]
    pub relation_type: String,
    #[serde(default = "default_strength")]
    pub strength: f64,
}

fn default_relation_type() -> String {
    "related".to_string()
}

fn default_strength() -> f64 {
    0.5
}
