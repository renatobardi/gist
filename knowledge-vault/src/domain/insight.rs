use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insight {
    pub id: String,
    pub summary: String,
    pub key_points: Vec<String>,
    pub raw_gemini_response: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GeminiResponse {
    pub summary: String,
    pub key_points: Vec<String>,
    pub concepts: Vec<GeminiConcept>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GeminiConcept {
    pub name: String,
    pub description: String,
    pub domain: String,
    pub relevance_weight: f64,
    #[serde(default)]
    pub related_concepts: Vec<GeminiRelatedConcept>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GeminiRelatedConcept {
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
