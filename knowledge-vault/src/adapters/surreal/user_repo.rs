use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use surrealdb::{engine::local::Db, Surreal};
use uuid::Uuid;

use crate::{
    domain::user::User,
    ports::repository::{RepoError, UserRepo},
};

#[derive(Debug, Serialize, Deserialize)]
struct UserRecord {
    id: Option<surrealdb::sql::Thing>,
    email: String,
    password_hash: String,
    role: String,
}

pub struct SurrealUserRepo {
    db: Surreal<Db>,
}

impl SurrealUserRepo {
    pub fn new(db: Surreal<Db>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl UserRepo for SurrealUserRepo {
    async fn count(&self) -> Result<u64, RepoError> {
        let mut result = self
            .db
            .query("SELECT count() FROM users GROUP ALL")
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

    async fn create(&self, email: String, password_hash: String) -> Result<User, RepoError> {
        let uuid = Uuid::new_v4().to_string();
        let id = format!("users:{uuid}");

        let record = UserRecord {
            id: None,
            email: email.clone(),
            password_hash: password_hash.clone(),
            role: "admin".to_string(),
        };

        let created: Option<UserRecord> = self
            .db
            .create(("users", uuid))
            .content(record)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("already exists") || msg.contains("UNIQUE") {
                    RepoError::EmailAlreadyExists
                } else {
                    RepoError::Internal(msg)
                }
            })?;

        let rec = created.ok_or_else(|| RepoError::Internal("no record returned".into()))?;

        Ok(User {
            id,
            email: rec.email,
            password_hash: rec.password_hash,
            role: rec.role,
        })
    }
}
