use crate::domain::user::User;

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
pub trait LoginAttemptRepo: Send + Sync {
    async fn record(&self, email: &str, succeeded: bool) -> Result<(), RepoError>;
    async fn count_recent_failures(&self, email: &str, window_seconds: u64) -> Result<u64, RepoError>;
    async fn oldest_recent_failure_ts(&self, email: &str, window_seconds: u64) -> Result<Option<i64>, RepoError>;
}
