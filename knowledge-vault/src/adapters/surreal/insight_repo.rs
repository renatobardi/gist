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
        work_id: &str,
        summary: &str,
        key_points: &[String],
        raw_gemini_response: &str,
    ) -> Result<Insight, RepoError> {
        let insight_id = Uuid::new_v4().to_string();

        let record = InsightRecord {
            id: None,
            summary: summary.to_string(),
            key_points: key_points.to_vec(),
            raw_gemini_response: raw_gemini_response.to_string(),
        };

        let created: Option<InsightRecord> = self
            .db
            .create(("insight", insight_id.clone()))
            .content(record)
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let rec =
            created.ok_or_else(|| RepoError::Internal("no insight record returned".into()))?;

        // Create interpreta edge: work -> insight
        self.db
            .query("RELATE type::thing('work', $work_id)->interpreta->type::thing('insight', $insight_id)")
            .bind(("work_id", work_id.to_string()))
            .bind(("insight_id", insight_id.clone()))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        Ok(Insight {
            id: insight_id,
            summary: rec.summary,
            key_points: rec.key_points,
            raw_gemini_response: rec.raw_gemini_response,
        })
    }

    async fn create_menciona(
        &self,
        insight_id: &str,
        concept_id: &str,
        relevance_weight: f64,
    ) -> Result<(), RepoError> {
        self.db
            .query("RELATE type::thing('insight', $insight_id)->menciona->type::thing('concept', $concept_id) SET relevance_weight = $weight")
            .bind(("insight_id", insight_id.to_string()))
            .bind(("concept_id", concept_id.to_string()))
            .bind(("weight", relevance_weight))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        Ok(())
    }
}
