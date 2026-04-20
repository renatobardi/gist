use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use surrealdb::{engine::local::Db, Surreal};
use uuid::Uuid;

fn thing_id_to_string(id: surrealdb::sql::Id) -> String {
    // SurrealDB wraps string IDs in backticks when using to_string().
    // Strip them to return the raw value.
    let s = id.to_string();
    s.trim_matches('`').to_string()
}

use crate::{
    domain::work::Work,
    ports::repository::{RepoError, WorkRepo},
};

#[derive(Debug, Serialize, Deserialize)]
struct WorkRecord {
    id: Option<surrealdb::sql::Thing>,
    title: String,
    author: String,
    isbn: Option<String>,
    open_library_id: Option<String>,
    status: String,
}

impl WorkRecord {
    fn into_work(self, fallback_id: Option<String>) -> Work {
        let id = self
            .id
            .map(|t| thing_id_to_string(t.id))
            .or(fallback_id)
            .unwrap_or_default();
        Work {
            id,
            title: self.title,
            author: self.author,
            isbn: self.isbn,
            open_library_id: self.open_library_id,
            status: self.status,
        }
    }
}

pub struct SurrealWorkRepo {
    db: Surreal<Db>,
}

impl SurrealWorkRepo {
    pub fn new(db: Surreal<Db>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl WorkRepo for SurrealWorkRepo {
    async fn find_by_isbn(&self, isbn: &str) -> Result<Option<Work>, RepoError> {
        let mut result = self
            .db
            .query("SELECT * FROM work WHERE isbn = $isbn LIMIT 1")
            .bind(("isbn", isbn.to_string()))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let records: Vec<WorkRecord> = result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        Ok(records.into_iter().next().map(|rec| rec.into_work(None)))
    }

    async fn create_work(&self, isbn: &str) -> Result<Work, RepoError> {
        let work_id = Uuid::new_v4().to_string();

        let record = WorkRecord {
            id: None,
            title: String::new(),
            author: String::new(),
            isbn: Some(isbn.to_string()),
            open_library_id: None,
            status: "pending".to_string(),
        };

        let created: Option<WorkRecord> = self
            .db
            .create(("work", work_id.clone()))
            .content(record)
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let rec = created.ok_or_else(|| RepoError::Internal("no record returned".into()))?;

        Ok(rec.into_work(Some(work_id)))
    }

    async fn find_by_open_library_id(&self, ol_id: &str) -> Result<Option<Work>, RepoError> {
        let mut result = self
            .db
            .query("SELECT * FROM work WHERE open_library_id = $ol_id LIMIT 1")
            .bind(("ol_id", ol_id.to_string()))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let records: Vec<WorkRecord> = result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        Ok(records.into_iter().next().map(|rec| rec.into_work(None)))
    }

    async fn create_work_by_title(
        &self,
        title: &str,
        author: &str,
        open_library_id: &str,
    ) -> Result<Work, RepoError> {
        let work_id = Uuid::new_v4().to_string();

        let record = WorkRecord {
            id: None,
            title: title.to_string(),
            author: author.to_string(),
            isbn: None,
            open_library_id: Some(open_library_id.to_string()),
            status: "pending".to_string(),
        };

        let created: Option<WorkRecord> = self
            .db
            .create(("work", work_id.clone()))
            .content(record)
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let rec = created.ok_or_else(|| RepoError::Internal("no record returned".into()))?;

        Ok(rec.into_work(Some(work_id)))
    }
}
