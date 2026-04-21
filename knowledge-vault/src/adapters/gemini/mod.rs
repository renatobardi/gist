use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::domain::insight::GeminiResponse;
use crate::ports::external::{BookMetadata, ExternalError, GeminiPort};

const GEMINI_TIMEOUT_SECS: u64 = 15;

pub struct GeminiClient {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl GeminiClient {
    pub fn new(api_key: String, model: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(GEMINI_TIMEOUT_SECS))
            .build()
            .expect("failed to build reqwest client for gemini");
        Self {
            client,
            api_key,
            model,
        }
    }
}

// ---- Gemini request shapes ----

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Serialize)]
struct GenerationConfig {
    response_mime_type: String,
    response_schema: serde_json::Value,
}

// ---- Gemini response shapes ----

#[derive(Deserialize)]
struct GeminiApiResponse {
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: CandidateContent,
}

#[derive(Deserialize)]
struct CandidateContent {
    parts: Vec<CandidatePart>,
}

#[derive(Deserialize)]
struct CandidatePart {
    text: String,
}

fn build_prompt(metadata: &BookMetadata) -> String {
    format!(
        "Analyze the following book and extract structured knowledge:\n\
        Title: {}\n\
        Author: {}\n\
        Description: {}\n\
        Subjects: {}\n\n\
        Extract a summary, key points, and concepts with their relationships.",
        metadata.title,
        metadata.author,
        metadata.description,
        metadata.subjects.join(", ")
    )
}

fn response_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "OBJECT",
        "properties": {
            "summary": {"type": "STRING"},
            "key_points": {"type": "ARRAY", "items": {"type": "STRING"}},
            "concepts": {
                "type": "ARRAY",
                "items": {
                    "type": "OBJECT",
                    "properties": {
                        "name": {"type": "STRING"},
                        "display_name": {"type": "STRING"},
                        "description": {"type": "STRING"},
                        "domain": {"type": "STRING"},
                        "relevance_weight": {"type": "NUMBER"},
                        "related_concepts": {
                            "type": "ARRAY",
                            "items": {
                                "type": "OBJECT",
                                "properties": {
                                    "name": {"type": "STRING"},
                                    "relation_type": {"type": "STRING"},
                                    "strength": {"type": "NUMBER"}
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

#[async_trait]
impl GeminiPort for GeminiClient {
    async fn extract_concepts(
        &self,
        metadata: &BookMetadata,
    ) -> Result<GeminiResponse, ExternalError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model
        );

        let body = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart {
                    text: build_prompt(metadata),
                }],
            }],
            generation_config: GenerationConfig {
                response_mime_type: "application/json".to_string(),
                response_schema: response_schema(),
            },
        };

        let resp = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ExternalError::Transient(format!("Gemini timeout: {e}"))
                } else {
                    ExternalError::Transient(format!("Gemini request error: {e}"))
                }
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let msg = format!("Gemini API error {status}: {body}");
            // 4xx errors (except 429 Too Many Requests) are permanent — retrying won't help
            return Err(
                if status.is_client_error() && status != reqwest::StatusCode::TOO_MANY_REQUESTS {
                    ExternalError::Permanent(msg)
                } else {
                    ExternalError::Transient(msg)
                },
            );
        }

        let api_resp: GeminiApiResponse = resp.json().await.map_err(|e| {
            ExternalError::Transient(format!("failed to parse Gemini response: {e}"))
        })?;

        let text = api_resp
            .candidates
            .into_iter()
            .next()
            .and_then(|c| c.content.parts.into_iter().next())
            .map(|p| p.text)
            .ok_or_else(|| {
                ExternalError::Permanent("Gemini returned empty candidates".to_string())
            })?;

        let gemini_response: GeminiResponse = serde_json::from_str(&text)
            .map_err(|e| ExternalError::Permanent(format!("Gemini schema violation: {e}")))?;

        // Validate required fields
        if gemini_response.summary.is_empty() {
            return Err(ExternalError::Permanent(
                "Gemini response missing required field: summary".to_string(),
            ));
        }

        Ok(gemini_response)
    }
}
