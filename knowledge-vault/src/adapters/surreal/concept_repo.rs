use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use surrealdb::{engine::local::Db, sql::Thing, Surreal};
use uuid::Uuid;

use crate::{
    domain::concept::{normalize_name, Concept},
    ports::repository::{ConceptRepo, RepoError},
};

#[derive(Debug, Serialize, Deserialize)]
struct ConceptRecord {
    id: Option<surrealdb::sql::Thing>,
    name: String,
    display_name: String,
    description: String,
    domain: String,
    created_at: Option<String>,
}

fn record_to_concept(rec: ConceptRecord) -> Concept {
    let id = rec
        .id
        .map(|t| {
            let s = t.id.to_string();
            s.trim_matches('`').to_string()
        })
        .unwrap_or_default();
    Concept {
        id,
        name: rec.name,
        display_name: rec.display_name,
        description: rec.description,
        domain: rec.domain,
        created_at: rec.created_at.unwrap_or_default(),
    }
}

pub struct SurrealConceptRepo {
    db: Surreal<Db>,
}

impl SurrealConceptRepo {
    pub fn new(db: Surreal<Db>) -> Self {
        Self { db }
    }

    pub async fn find_by_name(&self, normalized_name: &str) -> Result<Option<Concept>, RepoError> {
        let mut result = self
            .db
            .query("SELECT * FROM concept WHERE name = $name LIMIT 1")
            .bind(("name", normalized_name.to_string()))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let records: Vec<ConceptRecord> = result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        Ok(records.into_iter().next().map(record_to_concept))
    }
}

#[async_trait]
impl ConceptRepo for SurrealConceptRepo {
    async fn upsert(&self, display_name: &str, description: &str, domain: &str) -> Result<Concept, RepoError> {
        let normalized = normalize_name(display_name);

        if let Some(existing) = self.find_by_name(&normalized).await? {
            return Ok(existing);
        }

        let concept_id = Uuid::new_v4().to_string();
        let record = ConceptRecord {
            id: None,
            name: normalized.clone(),
            display_name: display_name.trim().to_string(),
            description: description.to_string(),
            domain: domain.to_string(),
            created_at: None,
        };

        match self.db.create(("concept", concept_id)).content(record).await {
            Ok(Some(rec)) => Ok(record_to_concept(rec)),
            Ok(None) => Err(RepoError::Internal("no concept record returned".into())),
            Err(e) => {
                let msg = e.to_string();
                // Concurrent upsert race: another caller won the UNIQUE index — return the winner.
                if msg.contains("already exists") || msg.contains("unique") {
                    self.find_by_name(&normalized).await?.ok_or_else(|| {
                        RepoError::Internal("concept not found after unique conflict".into())
                    })
                } else {
                    Err(RepoError::Internal(msg))
                }
            }
        }
    }

    async fn create_menciona_edge(
        &self,
        insight_id: &str,
        concept_id: &str,
        relevance_weight: f64,
    ) -> Result<(), RepoError> {
        let insight = Thing::from(("insight", insight_id));
        let concept = Thing::from(("concept", concept_id));
        self.db
            .query("RELATE $insight->menciona->$concept SET relevance_weight = $weight")
            .bind(("insight", insight))
            .bind(("concept", concept))
            .bind(("weight", relevance_weight))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn create_relacionado_a_edge(
        &self,
        from_concept_id: &str,
        to_concept_id: &str,
        relation_type: &str,
        strength: f64,
    ) -> Result<(), RepoError> {
        let from = Thing::from(("concept", from_concept_id));
        let to = Thing::from(("concept", to_concept_id));
        self.db
            .query("RELATE $from->relacionado_a->$to SET relation_type = $rtype, strength = $strength")
            .bind(("from", from))
            .bind(("to", to))
            .bind(("rtype", relation_type.to_string()))
            .bind(("strength", strength))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        Ok(())
    }
}
