use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use surrealdb::{engine::local::Db, sql::Thing, Surreal};
use uuid::Uuid;

use crate::domain::insight::{ConceptWithWeight, InsightDetail};
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

#[derive(Deserialize)]
struct ConceptEdgeRecord {
    out: ConceptNode,
    relevance_weight: f64,
}

#[derive(Deserialize)]
struct ConceptNode {
    id: Thing,
    display_name: String,
    description: String,
    domain: String,
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

    async fn get_for_work(&self, work_id: &str) -> Result<Option<InsightDetail>, RepoError> {
        Uuid::parse_str(work_id)
            .map_err(|_| RepoError::Internal(format!("invalid work_id format: {work_id}")))?;

        // Step 1: resolve the insight linked to this work via the interpreta edge
        let mut edge_result = self
            .db
            .query("SELECT out FROM interpreta WHERE in = type::thing('work', $id) LIMIT 1")
            .bind(("id", work_id.to_string()))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let edge_rows: Vec<OutRecord> = edge_result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let insight_thing = match edge_rows.into_iter().next() {
            Some(r) => r.out,
            None => return Ok(None),
        };

        let insight_id = thing_id_to_string(insight_thing.clone());

        // Step 2: fetch the insight record itself
        let insight_record: Option<InsightRecord> = self
            .db
            .select(("insight", insight_id.as_str()))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let insight = match insight_record {
            Some(r) => r,
            None => return Ok(None),
        };

        // Step 3: fetch concepts linked via menciona, including relevance_weight
        let mut concept_result = self
            .db
            .query("SELECT out, relevance_weight FROM menciona WHERE in = $insight_id FETCH out")
            .bind(("insight_id", insight_thing))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let concept_edges: Vec<ConceptEdgeRecord> = concept_result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let concepts = concept_edges
            .into_iter()
            .map(|edge| ConceptWithWeight {
                id: thing_id_to_string(edge.out.id),
                display_name: edge.out.display_name,
                description: edge.out.description,
                domain: edge.out.domain,
                relevance_weight: edge.relevance_weight,
            })
            .collect();

        Ok(Some(InsightDetail {
            id: insight_id,
            summary: insight.summary,
            key_points: insight.key_points,
            concepts,
        }))
    }
}
