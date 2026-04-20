use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use surrealdb::{engine::local::Db, Surreal};
use tracing::warn;
use uuid::Uuid;

use crate::{
    domain::concept::Concept,
    domain::insight::GeminiConcept,
    ports::repository::{ConceptEdge, ConceptRepo, RepoError},
};

#[derive(Debug, Serialize, Deserialize)]
struct ConceptRecord {
    id: Option<surrealdb::sql::Thing>,
    name: String,
    display_name: String,
    description: String,
    domain: String,
}

pub struct SurrealConceptRepo {
    db: Surreal<Db>,
}

impl SurrealConceptRepo {
    pub fn new(db: Surreal<Db>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ConceptRepo for SurrealConceptRepo {
    async fn upsert(&self, gemini_concept: &GeminiConcept) -> Result<Concept, RepoError> {
        let normalized = Concept::normalize_name(&gemini_concept.name);

        // Check if concept already exists by normalized name
        let mut result = self
            .db
            .query("SELECT * FROM concept WHERE name = $name LIMIT 1")
            .bind(("name", normalized.clone()))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let existing: Vec<ConceptRecord> = result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        if let Some(rec) = existing.into_iter().next() {
            let id = rec
                .id
                .map(|t| {
                    let s = t.id.to_string();
                    s.trim_matches('`').to_string()
                })
                .unwrap_or_default();
            return Ok(Concept {
                id,
                name: rec.name,
                display_name: rec.display_name,
                description: rec.description,
                domain: rec.domain,
            });
        }

        let concept_id = Uuid::new_v4().to_string();
        let record = ConceptRecord {
            id: None,
            name: normalized,
            display_name: gemini_concept.name.trim().to_string(),
            description: gemini_concept.description.clone(),
            domain: gemini_concept.domain.clone(),
        };

        let created: Option<ConceptRecord> = self
            .db
            .create(("concept", concept_id.clone()))
            .content(record)
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let rec = created.ok_or_else(|| RepoError::Internal("no concept record returned".into()))?;

        Ok(Concept {
            id: concept_id,
            name: rec.name,
            display_name: rec.display_name,
            description: rec.description,
            domain: rec.domain,
        })
    }

    async fn create_relacionado_a(&self, edge: ConceptEdge) -> Result<(), RepoError> {
        let from_norm = Concept::normalize_name(&edge.from_name);
        let to_norm_str = Concept::normalize_name(&edge.to_name);

        // Resolve concept IDs
        let mut from_result = self
            .db
            .query("SELECT id FROM concept WHERE name = $name LIMIT 1")
            .bind(("name", from_norm))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        #[derive(Deserialize)]
        struct IdRecord {
            id: surrealdb::sql::Thing,
        }

        let from_records: Vec<IdRecord> = from_result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        let mut to_result = self
            .db
            .query("SELECT id FROM concept WHERE name = $name LIMIT 1")
            .bind(("name", to_norm_str))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let to_records: Vec<IdRecord> = to_result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let from_rec = match from_records.into_iter().next() {
            Some(r) => r,
            None => {
                warn!(from = %edge.from_name, to = %edge.to_name, "skipping relacionado_a: source concept not found");
                return Ok(());
            }
        };
        let to_rec = match to_records.into_iter().next() {
            Some(r) => r,
            None => {
                warn!(from = %edge.from_name, to = %edge.to_name, "skipping relacionado_a: target concept not found");
                return Ok(());
            }
        };

        let from_id = {
            let s = from_rec.id.id.to_string();
            s.trim_matches('`').to_string()
        };
        let to_id = {
            let s = to_rec.id.id.to_string();
            s.trim_matches('`').to_string()
        };

        self.db
            .query("RELATE type::thing('concept', $from)->relacionado_a->type::thing('concept', $to) SET relation_type = $rel, strength = $strength")
            .bind(("from", from_id))
            .bind(("to", to_id))
            .bind(("rel", edge.relation_type))
            .bind(("strength", edge.strength))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        Ok(())
    }
}
