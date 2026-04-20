use crate::domain::concept::Concept;
use crate::domain::insight::Insight;
use crate::domain::user::{PersonalAccessToken, User};
use crate::domain::work::Work;

#[derive(Debug)]
pub enum RepoError {
    EmailAlreadyExists,
    NotFound,
    Internal(String),
}

impl std::fmt::Display for RepoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoError::EmailAlreadyExists => write!(f, "Email already exists"),
            RepoError::NotFound => write!(f, "Not found"),
            RepoError::Internal(msg) => write!(f, "Internal error: {msg}"),
        }
    }
}

#[async_trait::async_trait]
pub trait UserRepo: Send + Sync {
    async fn count(&self) -> Result<u64, RepoError>;
    async fn create(&self, email: String, password_hash: String) -> Result<User, RepoError>;
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, RepoError>;
}

#[async_trait::async_trait]
pub trait TokenRepo: Send + Sync {
    async fn create(
        &self,
        user_id: &str,
        name: String,
        token_hash: String,
    ) -> Result<String, RepoError>;
    async fn list(&self, user_id: &str) -> Result<Vec<PersonalAccessToken>, RepoError>;
    async fn find_by_token(&self, token: &str) -> Result<Option<PersonalAccessToken>, RepoError>;
    async fn revoke(&self, token_id: &str, user_id: &str) -> Result<(), RepoError>;
}

#[async_trait::async_trait]
pub trait WorkRepo: Send + Sync {
    async fn find_by_isbn(&self, isbn: &str) -> Result<Option<Work>, RepoError>;
    async fn create_work(&self, isbn: &str) -> Result<Work, RepoError>;
}

#[async_trait::async_trait]
pub trait ConceptRepo: Send + Sync {
    /// Upsert a concept by normalized name. First-write wins: if the name already exists,
    /// the existing record is returned without modification.
    async fn upsert(&self, display_name: &str, description: &str, domain: &str) -> Result<Concept, RepoError>;
    async fn find_by_name(&self, normalized_name: &str) -> Result<Option<Concept>, RepoError>;
    /// Create a menciona edge from an insight to a concept with a relevance weight.
    async fn create_menciona_edge(
        &self,
        insight_id: &str,
        concept_id: &str,
        relevance_weight: f64,
    ) -> Result<(), RepoError>;
    /// Create a relacionado_a edge between two concepts.
    async fn create_relacionado_a_edge(
        &self,
        from_concept_id: &str,
        to_concept_id: &str,
        relation_type: &str,
        strength: f64,
    ) -> Result<(), RepoError>;
}

#[async_trait::async_trait]
pub trait InsightRepo: Send + Sync {
    async fn create(
        &self,
        summary: &str,
        key_points: Vec<String>,
        raw_gemini_response: &str,
    ) -> Result<Insight, RepoError>;
    /// Create an interpreta edge from a work to an insight.
    async fn create_interpreta_edge(&self, work_id: &str, insight_id: &str) -> Result<(), RepoError>;
}

#[async_trait::async_trait]
pub trait LoginAttemptRepo: Send + Sync {
    async fn record(&self, email: &str, succeeded: bool) -> Result<(), RepoError>;
    async fn count_recent_failures(
        &self,
        email: &str,
        window_seconds: u64,
    ) -> Result<u64, RepoError>;
    async fn oldest_recent_failure_ts(
        &self,
        email: &str,
        window_seconds: u64,
    ) -> Result<Option<i64>, RepoError>;
}
