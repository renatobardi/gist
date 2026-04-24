use crate::domain::concept::{ConceptDetail, GraphData};
use crate::domain::insight::{ExtractedConcept, GeminiResponse, InsightDetail};
use crate::domain::user::{PersonalAccessToken, User, UserPreferences};
use crate::domain::work::Work;

#[derive(Debug, Clone, Copy)]
pub enum WorkSortField {
    Title,
    CreatedAt,
    ProgressPct,
}

#[derive(Debug, Clone, Copy)]
pub enum SortOrder {
    Asc,
    Desc,
}

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
    async fn find_by_id(&self, id: &str) -> Result<Option<User>, RepoError>;
    async fn update_profile(
        &self,
        id: &str,
        display_name: Option<String>,
        preferences: Option<UserPreferences>,
    ) -> Result<User, RepoError>;
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
    async fn delete_work_cascade(&self, id: &str) -> Result<(), RepoError>;
    async fn update_progress(
        &self,
        id: &str,
        progress_pct: i32,
        last_action: &str,
    ) -> Result<(), RepoError>;
    async fn update_google_books_metadata(
        &self,
        id: &str,
        cover_image_url: Option<&str>,
        page_count: Option<i32>,
        publisher: Option<&str>,
        average_rating: Option<f64>,
        preview_link: Option<&str>,
    ) -> Result<(), RepoError>;
    async fn update_reading_status(
        &self,
        id: &str,
        reading_status: Option<&str>,
    ) -> Result<Work, RepoError>;
    async fn list_works_filtered(
        &self,
        status: Option<&str>,
        domain: Option<&str>,
        sort: WorkSortField,
        order: SortOrder,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Work>, RepoError>;
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

    async fn get_for_work(&self, work_id: &str) -> Result<Option<InsightDetail>, RepoError>;
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

/// Atomically writes the full graph result for one work: insight node, interpreta edge,
/// concept upserts, menciona edges, relacionado_a edges, and work status → "done".
/// Uses SurrealDB BEGIN TRANSACTION / COMMIT to guarantee all-or-nothing semantics.
#[async_trait::async_trait]
pub trait GraphWriteRepo: Send + Sync {
    async fn write_graph_transaction(
        &self,
        work_id: &str,
        gemini_response: &GeminiResponse,
    ) -> Result<(), RepoError>;
}

/// Read-side graph queries: retrieve concepts and edges for the graph visualization.
#[async_trait::async_trait]
pub trait GraphReadRepo: Send + Sync {
    /// Returns all concepts (optionally filtered by domain) and the edges between them.
    async fn get_graph(&self, domains: Option<Vec<String>>) -> Result<GraphData, RepoError>;

    /// Returns the concept, the books that mention it, and its related concepts.
    async fn get_concept_detail(&self, id: &str) -> Result<Option<ConceptDetail>, RepoError>;
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
