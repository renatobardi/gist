use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use surrealdb::{engine::local::Db, Surreal};
use uuid::Uuid;

use crate::{
    domain::user::{User, UserPreferences},
    ports::repository::{RepoError, UserRepo},
};

fn thing_id_to_string(id: surrealdb::sql::Id) -> String {
    let s = id.to_string();
    s.trim_matches('`').to_string()
}

#[derive(Debug, Serialize, Deserialize)]
struct UserRecord {
    id: Option<surrealdb::sql::Thing>,
    email: String,
    password_hash: String,
    role: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    preferences: Option<serde_json::Value>,
}

fn record_to_user(rec: UserRecord) -> User {
    let id = rec.id.map(|t| thing_id_to_string(t.id)).unwrap_or_default();
    let preferences = rec.preferences.and_then(|v| serde_json::from_value(v).ok());
    User {
        id,
        email: rec.email,
        password_hash: rec.password_hash,
        role: rec.role,
        display_name: rec.display_name,
        preferences,
    }
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
        let user_id = Uuid::new_v4().to_string();

        let record = UserRecord {
            id: None,
            email: email.clone(),
            password_hash: password_hash.clone(),
            role: "admin".to_string(),
            display_name: None,
            preferences: None,
        };

        let created: Option<UserRecord> = self
            .db
            .create(("users", user_id.clone()))
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

        let mut rec = created.ok_or_else(|| RepoError::Internal("no record returned".into()))?;
        rec.id = None;
        let mut user = record_to_user(rec);
        user.id = user_id;
        Ok(user)
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>, RepoError> {
        let email_owned = email.to_string();
        let mut result = self
            .db
            .query("SELECT * FROM users WHERE email = $email LIMIT 1")
            .bind(("email", email_owned))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let records: Vec<UserRecord> = result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        Ok(records.into_iter().next().map(record_to_user))
    }

    async fn find_by_id(&self, _id: &str) -> Result<Option<User>, RepoError> {
        todo!("implement find_by_id")
    }

    async fn update_profile(
        &self,
        _id: &str,
        _display_name: Option<String>,
        _preferences: Option<UserPreferences>,
    ) -> Result<User, RepoError> {
        todo!("implement update_profile")
    }
}
