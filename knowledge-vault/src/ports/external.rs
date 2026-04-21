use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct OpenLibraryBook {
    pub open_library_id: String,
    pub title: String,
    pub author: String,
}

#[derive(Debug, Clone)]
pub struct BookMetadata {
    pub title: String,
    pub author: String,
    pub description: String,
    pub subjects: Vec<String>,
}

#[derive(Debug)]
pub enum ExternalError {
    Transient(String),
    Permanent(String),
}

impl std::fmt::Display for ExternalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExternalError::Transient(msg) => write!(f, "transient error: {msg}"),
            ExternalError::Permanent(msg) => write!(f, "permanent error: {msg}"),
        }
    }
}

#[async_trait]
pub trait OpenLibraryPort: Send + Sync {
    async fn search_by_title(&self, title: &str) -> Result<Option<OpenLibraryBook>, String>;
    async fn fetch_by_isbn(&self, isbn: &str) -> Result<BookMetadata, ExternalError>;
}

use crate::domain::insight::GeminiResponse;

#[async_trait]
pub trait GeminiPort: Send + Sync {
    async fn extract_concepts(
        &self,
        metadata: &BookMetadata,
    ) -> Result<GeminiResponse, ExternalError>;
}
