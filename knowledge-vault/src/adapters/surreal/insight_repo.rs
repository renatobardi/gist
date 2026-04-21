use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use surrealdb::{engine::local::Db, sql::Thing, Surreal};
use uuid::Uuid;

use crate::ports::repository::{InsightRepo, RepoError};

fn thing_id_to_string(thing: Thing) -> String {
    let s = thing.id.to_string();
    s.trim_matches('`').to_string()
}

#[derive(Deserialize)]
struct OutRecord {
    out: Thing,
}

pub struct SurrealInsightRepo {
    db: Surreal<Db>,
}

impl SurrealInsightRepo {
    pub fn new(db: Surreal<Db>) -> Self {
        Self { db }
    }
}

#[derive(Serialize)]
struct InsightContent {
    summary: String,
    key_points: Vec<String>,
    raw_gemini_response: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct InsightRecord {
    id: Option<Thing>,
    summary: String,
    key_points: Vec<String>,
    raw_gemini_response: String,
}

#[async_trait]
impl InsightRepo for SurrealInsightRepo {
    async fn create_insight(
        &self,
        work_id: &str,
        summary: &str,
        key_points: Vec<String>,
        raw_json: &str,
    ) -> Result<String, RepoError> {
        // Validate work_id is a UUID before embedding it in SurrealQL
        Uuid::parse_str(work_id)
            .map_err(|_| RepoError::Internal(format!("invalid work_id format: {work_id}")))?;

        // Idempotency: return existing insight if this work was already processed
        let mut check = self
            .db
            .query("SELECT out FROM interpreta WHERE in = type::thing('work', $id) LIMIT 1")
            .bind(("id", work_id.to_string()))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        let existing: Vec<OutRecord> = check
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        if let Some(rec) = existing.into_iter().next() {
            return Ok(thing_id_to_string(rec.out));
        }

        let insight_id = Uuid::new_v4().to_string();

        let content = InsightContent {
            summary: summary.to_string(),
            key_points,
            raw_gemini_response: raw_json.to_string(),
        };

        let _created: Option<InsightRecord> = self
            .db
            .create(("insight", insight_id.clone()))
            .content(content)
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        // UUIDs only contain [0-9a-f-]: no injection risk from embedding them directly.
        let relate_sql = format!("RELATE work:`{work_id}`->interpreta->insight:`{insight_id}`");
        self.db
            .query(&relate_sql)
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        Ok(insight_id)
    }
}
