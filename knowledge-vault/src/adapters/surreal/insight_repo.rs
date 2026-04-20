use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use surrealdb::{engine::local::Db, Surreal};
use uuid::Uuid;

use crate::{
    domain::insight::Insight,
    ports::repository::{InsightRepo, RepoError},
};

#[derive(Debug, Serialize, Deserialize)]
struct InsightRecord {
    id: Option<surrealdb::sql::Thing>,
    summary: String,
    key_points: Vec<String>,
    raw_gemini_response: String,
    created_at: Option<String>,
}

fn record_to_insight(rec: InsightRecord) -> Insight {
    let id = rec
        .id
        .map(|t| {
            let s = t.id.to_string();
            s.trim_matches('`').to_string()
        })
        .unwrap_or_default();
    Insight {
        id,
        summary: rec.summary,
        key_points: rec.key_points,
        raw_gemini_response: rec.raw_gemini_response,
        created_at: rec.created_at.unwrap_or_default(),
    }
}

pub struct SurrealInsightRepo {
    db: Surreal<Db>,
}

impl SurrealInsightRepo {
    pub fn new(db: Surreal<Db>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl InsightRepo for SurrealInsightRepo {
    async fn create(
        &self,
        summary: &str,
        key_points: Vec<String>,
        raw_gemini_response: &str,
    ) -> Result<Insight, RepoError> {
        let insight_id = Uuid::new_v4().to_string();
        let record = InsightRecord {
            id: None,
            summary: summary.to_string(),
            key_points,
            raw_gemini_response: raw_gemini_response.to_string(),
            created_at: None,
        };

        let created: Option<InsightRecord> = self
            .db
            .create(("insight", insight_id))
            .content(record)
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        created
            .map(record_to_insight)
            .ok_or_else(|| RepoError::Internal("no insight record returned".into()))
    }

    async fn create_interpreta_edge(&self, work_id: &str, insight_id: &str) -> Result<(), RepoError> {
        self.db
            .query("RELATE $work->interpreta->$insight")
            .bind(("work", format!("work:{work_id}")))
            .bind(("insight", format!("insight:{insight_id}")))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        Ok(())
    }
}
