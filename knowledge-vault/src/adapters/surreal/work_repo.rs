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
    ports::repository::{RepoError, SortOrder, WorkRepo, WorkSortField},
};

#[derive(Debug, Serialize, Deserialize)]
struct WorkRecord {
    id: Option<surrealdb::sql::Thing>,
    title: String,
    author: String,
    isbn: Option<String>,
    open_library_id: Option<String>,
    status: String,
    error_msg: Option<String>,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    progress_pct: Option<serde_json::Value>,
    #[serde(default)]
    last_action: Option<serde_json::Value>,
    #[serde(default)]
    reading_status: Option<String>,
    #[serde(default)]
    cover_image_url: Option<String>,
    #[serde(default)]
    page_count: Option<i32>,
    #[serde(default)]
    publisher: Option<String>,
    #[serde(default)]
    average_rating: Option<f64>,
    #[serde(default)]
    preview_link: Option<String>,
}

fn record_to_work(rec: WorkRecord, id_override: Option<String>) -> Work {
    let id =
        id_override.unwrap_or_else(|| rec.id.map(|t| thing_id_to_string(t.id)).unwrap_or_default());
    let progress_pct = rec
        .progress_pct
        .and_then(|v| v.as_f64())
        .map(|f| f as i32)
        .unwrap_or(0);
    let last_action = rec
        .last_action
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();
    Work {
        id,
        title: rec.title,
        author: rec.author,
        isbn: rec.isbn,
        open_library_id: rec.open_library_id,
        status: rec.status,
        error_msg: rec.error_msg,
        created_at: rec.created_at,
        updated_at: rec.updated_at,
        progress_pct,
        last_action,
        reading_status: rec.reading_status,
        cover_image_url: rec.cover_image_url,
        page_count: rec.page_count,
        publisher: rec.publisher,
        average_rating: rec.average_rating,
        preview_link: rec.preview_link,
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
    async fn find_by_id(&self, work_id: &str) -> Result<Option<Work>, RepoError> {
        let record: Option<WorkRecord> = self
            .db
            .select(("work", work_id))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        Ok(record.map(|rec| record_to_work(rec, Some(work_id.to_string()))))
    }

    async fn update_status(
        &self,
        work_id: &str,
        status: &str,
        error_msg: Option<&str>,
    ) -> Result<(), RepoError> {
        self.db
            .query(
                "UPDATE type::thing('work', $id) SET status = $status, error_msg = $error_msg, updated_at = time::now()",
            )
            .bind(("id", work_id.to_string()))
            .bind(("status", status.to_string()))
            .bind(("error_msg", error_msg.map(|s| s.to_string())))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        Ok(())
    }

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

        Ok(records
            .into_iter()
            .next()
            .map(|rec| record_to_work(rec, None)))
    }

    async fn create_work(&self, isbn: &str) -> Result<Work, RepoError> {
        let work_id = Uuid::new_v4().to_string();

        let created: Option<WorkRecord> = self
            .db
            .create(("work", work_id.clone()))
            .content(serde_json::json!({
                "isbn": isbn,
                "title": "",
                "author": "",
                "status": "pending",
            }))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let rec = created.ok_or_else(|| RepoError::Internal("no record returned".into()))?;

        Ok(record_to_work(rec, Some(work_id)))
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

        Ok(records
            .into_iter()
            .next()
            .map(|rec| record_to_work(rec, None)))
    }

    async fn create_work_by_title(
        &self,
        title: &str,
        author: &str,
        open_library_id: &str,
    ) -> Result<Work, RepoError> {
        let work_id = Uuid::new_v4().to_string();

        let created: Option<WorkRecord> = self
            .db
            .create(("work", work_id.clone()))
            .content(serde_json::json!({
                "title": title,
                "author": author,
                "open_library_id": open_library_id,
                "status": "pending",
            }))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let rec = created.ok_or_else(|| RepoError::Internal("no record returned".into()))?;

        Ok(record_to_work(rec, Some(work_id)))
    }

    async fn list_works(&self, limit: u32, offset: u32) -> Result<Vec<Work>, RepoError> {
        let mut result = self
            .db
            .query("SELECT * FROM work ORDER BY created_at DESC LIMIT $limit START $offset")
            .bind(("limit", limit))
            .bind(("offset", offset))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let records: Vec<WorkRecord> = result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        Ok(records
            .into_iter()
            .map(|rec| record_to_work(rec, None))
            .collect())
    }

    async fn get_work_by_id(&self, id: &str) -> Result<Option<Work>, RepoError> {
        let record: Option<WorkRecord> = self
            .db
            .select(("work", id))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        Ok(record.map(|rec| record_to_work(rec, Some(id.to_string()))))
    }

    async fn update_work_status(
        &self,
        id: &str,
        status: &str,
        error_msg: Option<&str>,
    ) -> Result<(), RepoError> {
        let updated: Option<WorkRecord> = self
            .db
            .query(
                "UPDATE work SET status = $status, error_msg = $error_msg, updated_at = time::now() WHERE id = type::thing('work', $id)",
            )
            .bind(("id", id.to_string()))
            .bind(("status", status.to_string()))
            .bind(("error_msg", error_msg.map(|s| s.to_string())))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        if updated.is_none() {
            return Err(RepoError::NotFound);
        }

        Ok(())
    }

    async fn reset_to_pending(&self, id: &str) -> Result<Work, RepoError> {
        let mut result = self
            .db
            .query(
                "UPDATE type::thing('work', $id) SET status = 'pending', updated_at = time::now() WHERE status = 'failed' RETURN AFTER",
            )
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let records: Vec<WorkRecord> = result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        records
            .into_iter()
            .next()
            .map(|rec| record_to_work(rec, Some(id.to_string())))
            .ok_or(RepoError::NotFound)
    }

    async fn delete_work_cascade(&self, _id: &str) -> Result<(), RepoError> {
        todo!("implement delete_work_cascade")
    }

    async fn update_progress(
        &self,
        id: &str,
        progress_pct: i32,
        last_action: &str,
    ) -> Result<(), RepoError> {
        self.db
            .query(
                "UPDATE type::thing('work', $id) SET progress_pct = $pct, last_action = $action, updated_at = time::now()",
            )
            .bind(("id", id.to_string()))
            .bind(("pct", progress_pct))
            .bind(("action", last_action.to_string()))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn update_google_books_metadata(
        &self,
        id: &str,
        cover_image_url: Option<&str>,
        page_count: Option<i32>,
        publisher: Option<&str>,
        average_rating: Option<f64>,
        preview_link: Option<&str>,
    ) -> Result<(), RepoError> {
        self.db
            .query(
                "UPDATE type::thing('work', $id) SET cover_image_url = $cover, page_count = $page_count, publisher = $publisher, average_rating = $rating, preview_link = $preview, updated_at = time::now()",
            )
            .bind(("id", id.to_string()))
            .bind(("cover", cover_image_url.map(|s| s.to_string())))
            .bind(("page_count", page_count))
            .bind(("publisher", publisher.map(|s| s.to_string())))
            .bind(("rating", average_rating))
            .bind(("preview", preview_link.map(|s| s.to_string())))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn update_reading_status(
        &self,
        _id: &str,
        _reading_status: Option<&str>,
    ) -> Result<Work, RepoError> {
        todo!("implement update_reading_status")
    }

    async fn list_works_filtered(
        &self,
        _status: Option<&str>,
        _domain: Option<&str>,
        _sort: WorkSortField,
        _order: SortOrder,
        _limit: u32,
        _offset: u32,
    ) -> Result<Vec<Work>, RepoError> {
        todo!("implement list_works_filtered")
    }
}
