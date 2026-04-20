use serde::{Deserialize, Serialize};

use crate::domain::concept::GeminiConcept;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insight {
    pub id: String,
    pub summary: String,
    pub key_points: Vec<String>,
    /// Full Gemini JSON response stored as a string for auditability.
    pub raw_gemini_response: String,
    pub created_at: String,
}

/// Full structured response from the Gemini API after concept extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiResponse {
    pub summary: String,
    pub key_points: Vec<String>,
    pub concepts: Vec<GeminiConcept>,
}
