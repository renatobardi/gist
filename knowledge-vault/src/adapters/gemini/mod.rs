use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    domain::insight::GeminiResponse,
    ports::external::{BookMetadata, GeminiError, GeminiPort},
};

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com";
const DEFAULT_MODEL: &str = "gemini-2.0-flash";
const TIMEOUT_SECS: u64 = 15;

pub struct GeminiAdapter {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl GeminiAdapter {
    pub fn new(api_key: impl Into<String>) -> Self {
        let model = std::env::var("KV_GEMINI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        Self::with_model_and_base(api_key, model, GEMINI_API_BASE)
    }

    pub fn with_model_and_base(
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
            .build()
            .expect("failed to build Gemini HTTP client");
        Self {
            client,
            api_key: api_key.into(),
            model: model.into(),
            base_url: base_url.into(),
        }
    }
}

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Serialize)]
struct Part {
    text: String,
}

#[derive(Serialize)]
struct GenerationConfig {
    #[serde(rename = "responseMimeType")]
    response_mime_type: String,
    #[serde(rename = "responseSchema")]
    response_schema: Value,
}

fn response_schema() -> Value {
    serde_json::json!({
        "type": "OBJECT",
        "properties": {
            "summary": {
                "type": "STRING",
                "description": "Executive summary of the book in 2-4 sentences."
            },
            "key_points": {
                "type": "ARRAY",
                "items": { "type": "STRING" },
                "description": "3-7 most important takeaways from the book."
            },
            "concepts": {
                "type": "ARRAY",
                "items": {
                    "type": "OBJECT",
                    "properties": {
                        "name": { "type": "STRING", "description": "Concept name, 1-5 words, title case." },
                        "description": { "type": "STRING", "description": "One sentence definition." },
                        "domain": { "type": "STRING", "description": "Primary knowledge domain, e.g. Economics, Computer Science, Philosophy." },
                        "relevance_weight": { "type": "NUMBER", "description": "Relevance to this book, 0.0-1.0." },
                        "related_concepts": {
                            "type": "ARRAY",
                            "items": {
                                "type": "OBJECT",
                                "properties": {
                                    "name": { "type": "STRING" },
                                    "relation_type": {
                                        "type": "STRING",
                                        "enum": ["enables", "contrasts_with", "is_part_of", "extends", "related"]
                                    },
                                    "strength": { "type": "NUMBER", "description": "Relation strength, 0.0-1.0." }
                                },
                                "required": ["name", "relation_type", "strength"]
                            }
                        }
                    },
                    "required": ["name", "description", "domain", "relevance_weight", "related_concepts"]
                },
                "description": "5-15 key concepts extracted from the book with their relationships."
            }
        },
        "required": ["summary", "key_points", "concepts"]
    })
}

fn build_prompt(metadata: &BookMetadata) -> String {
    let subjects = if metadata.subjects.is_empty() {
        "none provided".to_string()
    } else {
        metadata.subjects.join(", ")
    };

    let description = if metadata.description.is_empty() {
        "No description available.".to_string()
    } else {
        metadata.description.clone()
    };

    format!(
        "Analyze the following book and extract key concepts with their relationships.\n\n\
        Title: {title}\n\
        Author: {author}\n\
        Description: {description}\n\
        Subject tags: {subjects}\n\n\
        Extract 5-15 key concepts that are central to understanding this book. \
        For each concept provide a clear one-sentence definition, identify its primary knowledge domain, \
        assign a relevance weight (0.0-1.0) based on how central it is to the book, \
        and list related concepts with the type of relationship and its strength.",
        title = metadata.title,
        author = metadata.author,
        description = description,
        subjects = subjects,
    )
}

#[derive(Deserialize)]
struct GeminiApiResponse {
    candidates: Option<Vec<Candidate>>,
    error: Option<GeminiApiError>,
}

#[derive(Deserialize)]
struct Candidate {
    content: Option<CandidateContent>,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct CandidateContent {
    parts: Vec<CandidatePart>,
}

#[derive(Deserialize)]
struct CandidatePart {
    text: Option<String>,
}

#[derive(Deserialize)]
struct GeminiApiError {
    code: Option<u16>,
    message: Option<String>,
}

#[async_trait]
impl GeminiPort for GeminiAdapter {
    async fn extract_concepts(&self, metadata: &BookMetadata) -> Result<GeminiResponse, GeminiError> {
        let url = format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            self.base_url, self.model, self.api_key
        );

        let request_body = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part {
                    text: build_prompt(metadata),
                }],
            }],
            generation_config: GenerationConfig {
                response_mime_type: "application/json".to_string(),
                response_schema: response_schema(),
            },
        };

        let response = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    GeminiError::Transient(format!("Gemini request timed out: {e}"))
                } else {
                    GeminiError::Transient(format!("Gemini request failed: {e}"))
                }
            })?;

        let status = response.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(GeminiError::Transient(
                "Gemini rate limit exceeded (429)".to_string(),
            ));
        }

        if status.is_server_error() {
            return Err(GeminiError::Transient(format!(
                "Gemini server error: {status}"
            )));
        }

        let api_resp: GeminiApiResponse = response
            .json()
            .await
            .map_err(|e| GeminiError::Transient(format!("Gemini response parse failed: {e}")))?;

        if let Some(err) = api_resp.error {
            let msg = err.message.unwrap_or_else(|| "unknown error".to_string());
            let code = err.code.unwrap_or(0);
            if code == 429 || code >= 500 {
                return Err(GeminiError::Transient(format!("Gemini API error {code}: {msg}")));
            }
            return Err(GeminiError::Permanent(format!("Gemini API error {code}: {msg}")));
        }

        let candidates = api_resp
            .candidates
            .ok_or_else(|| GeminiError::Permanent("Gemini returned no candidates".to_string()))?;

        let candidate = candidates
            .into_iter()
            .next()
            .ok_or_else(|| GeminiError::Permanent("Gemini candidates list is empty".to_string()))?;

        if let Some(reason) = &candidate.finish_reason {
            if reason == "SAFETY" || reason == "RECITATION" {
                return Err(GeminiError::Permanent(format!(
                    "Gemini refused to generate content: {reason}"
                )));
            }
        }

        let content = candidate
            .content
            .ok_or_else(|| GeminiError::Permanent("Gemini candidate has no content".to_string()))?;

        let raw_text = content
            .parts
            .into_iter()
            .filter_map(|p| p.text)
            .collect::<Vec<_>>()
            .join("");

        let gemini_response: GeminiResponse = serde_json::from_str(&raw_text).map_err(|e| {
            GeminiError::Permanent(format!(
                "Gemini response violates schema: {e}. Raw: {raw_text}"
            ))
        })?;

        // Validate required fields on concepts
        for concept in &gemini_response.concepts {
            if concept.name.trim().is_empty() {
                return Err(GeminiError::Permanent(
                    "Gemini concept missing required field: name".to_string(),
                ));
            }
            if concept.description.trim().is_empty() {
                return Err(GeminiError::Permanent(
                    "Gemini concept missing required field: description".to_string(),
                ));
            }
            if concept.domain.trim().is_empty() {
                return Err(GeminiError::Permanent(
                    "Gemini concept missing required field: domain".to_string(),
                ));
            }
        }

        Ok(gemini_response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::external::BookMetadata;

    fn make_metadata() -> BookMetadata {
        BookMetadata {
            title: "Clean Code".to_string(),
            author: "Robert C. Martin".to_string(),
            description: "A handbook of agile software craftsmanship.".to_string(),
            subjects: vec!["Software Engineering".to_string(), "Agile".to_string()],
            open_library_id: None,
        }
    }

    #[tokio::test]
    async fn extracts_concepts_from_valid_response() {
        let mut server = mockito::Server::new_async().await;

        let body = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "text": serde_json::json!({
                            "summary": "A book about writing clean, maintainable code.",
                            "key_points": ["Write readable code", "Avoid duplication", "Keep functions small"],
                            "concepts": [{
                                "name": "Clean Code",
                                "description": "Code that is easy to read and maintain.",
                                "domain": "Software Engineering",
                                "relevance_weight": 1.0,
                                "related_concepts": [{
                                    "name": "Refactoring",
                                    "relation_type": "related",
                                    "strength": 0.9
                                }]
                            }]
                        }).to_string()
                    }]
                },
                "finishReason": "STOP"
            }]
        });

        let _mock = server
            .mock("POST", mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body.to_string())
            .create_async()
            .await;

        let adapter = GeminiAdapter::with_model_and_base("test-key", "gemini-test", server.url());
        let result = adapter.extract_concepts(&make_metadata()).await;

        assert!(result.is_ok(), "expected Ok, got: {result:?}");
        let resp = result.unwrap();
        assert_eq!(resp.concepts.len(), 1);
        assert_eq!(resp.concepts[0].name, "Clean Code");
    }

    #[tokio::test]
    async fn returns_permanent_error_on_schema_violation() {
        let mut server = mockito::Server::new_async().await;

        let body = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{ "text": "{\"invalid\": true}" }]
                },
                "finishReason": "STOP"
            }]
        });

        let _mock = server
            .mock("POST", mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body.to_string())
            .create_async()
            .await;

        let adapter = GeminiAdapter::with_model_and_base("test-key", "gemini-test", server.url());
        let result = adapter.extract_concepts(&make_metadata()).await;

        assert!(matches!(result, Err(GeminiError::Permanent(_))));
    }

    #[tokio::test]
    async fn returns_transient_error_on_429() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", mockito::Matcher::Any)
            .with_status(429)
            .with_body("{}")
            .create_async()
            .await;

        let adapter = GeminiAdapter::with_model_and_base("test-key", "gemini-test", server.url());
        let result = adapter.extract_concepts(&make_metadata()).await;

        assert!(matches!(result, Err(GeminiError::Transient(_))));
    }
}
