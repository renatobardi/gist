use crate::domain::insight::GeminiResponse;
use async_trait::async_trait;

pub struct BookMetadata {
    pub title: String,
    pub author: String,
    pub description: String,
    pub subjects: Vec<String>,
    pub open_library_id: Option<String>,
}

#[async_trait]
pub trait OpenLibraryPort: Send + Sync {
    async fn fetch_metadata(&self, isbn: &str) -> Result<Option<BookMetadata>, String>;
}

#[derive(Debug)]
pub enum GeminiError {
    Transient(String),
    Permanent(String),
}

impl std::fmt::Display for GeminiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeminiError::Transient(msg) => write!(f, "transient: {msg}"),
            GeminiError::Permanent(msg) => write!(f, "permanent: {msg}"),
        }
    }
}

#[async_trait]
pub trait GeminiPort: Send + Sync {
    /// Returns the parsed response and the raw JSON text from Gemini (before struct deserialization).
    async fn extract_concepts(
        &self,
        metadata: &BookMetadata,
    ) -> Result<(GeminiResponse, String), GeminiError>;
}
