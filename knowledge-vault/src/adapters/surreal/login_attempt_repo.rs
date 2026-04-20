use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use surrealdb::{engine::local::Db, Surreal};
use uuid::Uuid;

use crate::ports::repository::{LoginAttemptRepo, RepoError};

#[derive(Debug, Serialize, Deserialize)]
struct LoginAttemptRecord {
    id: Option<surrealdb::sql::Thing>,
    email: String,
    succeeded: bool,
    attempted_at: Option<surrealdb::sql::Datetime>,
}

pub struct SurrealLoginAttemptRepo {
    db: Surreal<Db>,
}

impl SurrealLoginAttemptRepo {
    pub fn new(db: Surreal<Db>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl LoginAttemptRepo for SurrealLoginAttemptRepo {
    async fn record(&self, email: &str, succeeded: bool) -> Result<(), RepoError> {
        let record = LoginAttemptRecord {
            id: None,
            email: email.to_string(),
            succeeded,
            attempted_at: None,
        };

        self.db
            .create::<Option<LoginAttemptRecord>>(("login_attempts", Uuid::new_v4().to_string()))
            .content(record)
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn count_recent_failures(
        &self,
        email: &str,
        window_seconds: u64,
    ) -> Result<u64, RepoError> {
        let query = format!(
            "SELECT count() FROM login_attempts WHERE email = $email AND succeeded = false AND attempted_at > time::now() - {}s GROUP ALL",
            window_seconds
        );
        let email_owned = email.to_string();

        let mut result = self
            .db
            .query(query)
            .bind(("email", email_owned))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let count: Option<serde_json::Value> = result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let n = count
            .as_ref()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        Ok(n)
    }

    async fn oldest_recent_failure_ts(
        &self,
        email: &str,
        window_seconds: u64,
    ) -> Result<Option<i64>, RepoError> {
        let query = format!(
            "SELECT attempted_at FROM login_attempts WHERE email = $email AND succeeded = false AND attempted_at > time::now() - {}s ORDER BY attempted_at ASC LIMIT 1",
            window_seconds
        );
        let email_owned = email.to_string();

        let mut result = self
            .db
            .query(query)
            .bind(("email", email_owned))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let records: Vec<serde_json::Value> = result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        if let Some(record) = records.into_iter().next() {
            if let Some(ts_str) = record.get("attempted_at").and_then(|v| v.as_str()) {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts_str) {
                    return Ok(Some(dt.timestamp()));
                }
            }
        }

        Ok(None)
    }
}
