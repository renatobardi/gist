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
    isbn: String,
    status: String,
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

        Ok(records.into_iter().next().map(|rec| Work {
            id: rec.id.map(|t| thing_id_to_string(t.id)).unwrap_or_default(),
            isbn: rec.isbn,
            status: rec.status,
        }))
    }

    async fn update_status(
        &self,
        work_id: &str,
        status: &str,
        error_msg: Option<&str>,
    ) -> Result<(), RepoError> {
        self.db
            .query("UPDATE type::thing('work', $id) SET status = $status, error_msg = $error_msg, updated_at = time::now()")
            .bind(("id", work_id.to_string()))
            .bind(("status", status.to_string()))
            .bind(("error_msg", error_msg.map(|s| s.to_string())))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn update_metadata(
        &self,
        work_id: &str,
        title: &str,
        author: &str,
        open_library_id: Option<&str>,
    ) -> Result<(), RepoError> {
        self.db
            .query("UPDATE type::thing('work', $id) SET title = $title, author = $author, open_library_id = $ol_id, updated_at = time::now()")
            .bind(("id", work_id.to_string()))
            .bind(("title", title.to_string()))
            .bind(("author", author.to_string()))
            .bind(("ol_id", open_library_id.map(|s| s.to_string())))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn create_work(&self, isbn: &str) -> Result<Work, RepoError> {
        let work_id = Uuid::new_v4().to_string();

        let record = WorkRecord {
            id: None,
            isbn: isbn.to_string(),
            status: "pending".to_string(),
        };

        let created: Option<WorkRecord> = self
            .db
            .create(("work", work_id.clone()))
            .content(record)
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let rec = created.ok_or_else(|| RepoError::Internal("no record returned".into()))?;

        Ok(Work {
            id: work_id,
            isbn: rec.isbn,
            status: rec.status,
        })
    }
}
