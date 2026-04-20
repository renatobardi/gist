use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::{engine::local::Db, Surreal};
use uuid::Uuid;

use crate::{
    domain::user::PersonalAccessToken,
    ports::repository::{RepoError, TokenRepo},
};

fn record_id_to_string(thing: Option<surrealdb::sql::Thing>) -> Result<String, RepoError> {
    thing
        .map(|t| t.id.to_string())
        .ok_or_else(|| RepoError::Internal("record returned without an ID".into()))
}

#[derive(Debug, Serialize, Deserialize)]
struct PatRecord {
    id: Option<surrealdb::sql::Thing>,
    user_id: String,
    name: String,
    token_hash: String,
    created_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
}

impl PatRecord {
    fn into_domain(self) -> Result<PersonalAccessToken, RepoError> {
        Ok(PersonalAccessToken {
            id: record_id_to_string(self.id)?,
            user_id: self.user_id,
            name: self.name,
            created_at: self
                .created_at
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default(),
            revoked_at: self.revoked_at.map(|dt| dt.to_rfc3339()),
        })
    }
}

pub struct SurrealTokenRepo {
    db: Surreal<Db>,
}

impl SurrealTokenRepo {
    pub fn new(db: Surreal<Db>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TokenRepo for SurrealTokenRepo {
    async fn create(
        &self,
        user_id: &str,
        name: String,
        token_hash: String,
    ) -> Result<String, RepoError> {
        let token_id = Uuid::new_v4().to_string();

        #[derive(Serialize)]
        struct NewPat {
            user_id: String,
            name: String,
            token_hash: String,
        }

        let created: Option<PatRecord> = self
            .db
            .create(("personal_access_tokens", token_id.clone()))
            .content(NewPat {
                user_id: user_id.to_string(),
                name,
                token_hash,
            })
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        created
            .ok_or_else(|| RepoError::Internal("no record returned".into()))
            .map(|_| token_id)
    }

    async fn list(&self, user_id: &str) -> Result<Vec<PersonalAccessToken>, RepoError> {
        let user_id_owned = user_id.to_string();
        let mut result = self
            .db
            .query("SELECT * FROM personal_access_tokens WHERE user_id = $user_id AND revoked_at IS NONE ORDER BY created_at DESC")
            .bind(("user_id", user_id_owned))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let records: Vec<PatRecord> = result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        records.into_iter().map(|r| r.into_domain()).collect()
    }

    async fn find_by_token(&self, token: &str) -> Result<Option<PersonalAccessToken>, RepoError> {
        let mut result = self
            .db
            .query("SELECT * FROM personal_access_tokens WHERE revoked_at IS NONE")
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let records: Vec<PatRecord> = result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        for record in records {
            if crate::domain::user::verify_pat(token, &record.token_hash) {
                return record.into_domain().map(Some);
            }
        }

        Ok(None)
    }

    async fn revoke(&self, token_id: &str, user_id: &str) -> Result<(), RepoError> {
        let record: Option<PatRecord> = self
            .db
            .select(("personal_access_tokens", token_id))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        match record {
            None => return Err(RepoError::NotFound),
            Some(ref r) if r.user_id != user_id => return Err(RepoError::NotFound),
            _ => {}
        }

        self.db
            .query("UPDATE type::thing('personal_access_tokens', $id) SET revoked_at = time::now()")
            .bind(("id", token_id.to_string()))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        Ok(())
    }
}
