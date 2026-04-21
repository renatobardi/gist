use crate::domain::insight::ExtractedConcept;
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
    async fn find_by_id(&self, id: &str) -> Result<Option<Work>, RepoError>;
    async fn create_work(&self, isbn: &str) -> Result<Work, RepoError>;
    async fn find_by_open_library_id(&self, ol_id: &str) -> Result<Option<Work>, RepoError>;
    async fn create_work_by_title(
        &self,
        title: &str,
        author: &str,
        open_library_id: &str,
    ) -> Result<Work, RepoError>;
    async fn list_works(&self, limit: u32, offset: u32) -> Result<Vec<Work>, RepoError>;
    async fn get_work_by_id(&self, id: &str) -> Result<Option<Work>, RepoError>;
    async fn update_work_status(
        &self,
        id: &str,
        status: &str,
        error_msg: Option<&str>,
    ) -> Result<(), RepoError>;
    async fn update_status(
        &self,
        work_id: &str,
        status: &str,
        error_msg: Option<&str>,
    ) -> Result<(), RepoError>;
    async fn reset_to_pending(&self, id: &str) -> Result<Work, RepoError>;
}

#[async_trait::async_trait]
pub trait InsightRepo: Send + Sync {
    async fn create_insight(
        &self,
        work_id: &str,
        summary: &str,
        key_points: Vec<String>,
        raw_json: &str,
    ) -> Result<String, RepoError>;
}

#[async_trait::async_trait]
pub trait ConceptRepo: Send + Sync {
    async fn upsert_and_link(
        &self,
        work_id: &str,
        insight_id: &str,
        concepts: Vec<ExtractedConcept>,
    ) -> Result<(), RepoError>;
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
