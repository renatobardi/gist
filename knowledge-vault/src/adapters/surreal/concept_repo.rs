use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use surrealdb::{engine::local::Db, sql::Thing, Surreal};
use uuid::Uuid;

use crate::domain::concept::normalize_concept_name;
use crate::domain::insight::ExtractedConcept;
use crate::ports::repository::{ConceptRepo, RepoError};

pub struct SurrealConceptRepo {
    db: Surreal<Db>,
}

impl SurrealConceptRepo {
    pub fn new(db: Surreal<Db>) -> Self {
        Self { db }
    }
}

#[derive(Serialize)]
struct ConceptContent {
    name: String,
    display_name: String,
    description: String,
    domain: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ConceptRecord {
    id: Option<Thing>,
    name: String,
    display_name: String,
    description: String,
    domain: String,
}

#[derive(Deserialize)]
struct IdOnlyRecord {
    id: Thing,
}

fn thing_id_to_string(thing: Thing) -> String {
    let s = thing.id.to_string();
    s.trim_matches('`').to_string()
}

/// Looks up a concept by normalized name; creates it if absent. Returns the bare UUID string.
async fn upsert_concept(
    db: &Surreal<Db>,
    name: &str,
    display_name: &str,
    description: &str,
    domain: &str,
) -> Result<String, RepoError> {
    let mut result = db
        .query("SELECT id FROM concept WHERE name = $name LIMIT 1")
        .bind(("name", name.to_string()))
        .await
        .map_err(|e| RepoError::Internal(e.to_string()))?;

    let existing: Vec<IdOnlyRecord> = result
        .take(0)
        .map_err(|e| RepoError::Internal(e.to_string()))?;

    if let Some(rec) = existing.into_iter().next() {
        return Ok(thing_id_to_string(rec.id));
    }

    let concept_id = Uuid::new_v4().to_string();
    let content = ConceptContent {
        name: name.to_string(),
        display_name: display_name.to_string(),
        description: description.to_string(),
        domain: domain.to_string(),
    };

    let _created: Option<ConceptRecord> = db
        .create(("concept", concept_id.clone()))
        .content(content)
        .await
        .map_err(|e| RepoError::Internal(e.to_string()))?;

    Ok(concept_id)
}

#[async_trait]
impl ConceptRepo for SurrealConceptRepo {
    async fn upsert_and_link(
        &self,
        _work_id: &str,
        insight_id: &str,
        concepts: Vec<ExtractedConcept>,
    ) -> Result<(), RepoError> {
        for concept in &concepts {
            let name = normalize_concept_name(&concept.display_name);

            let concept_id = upsert_concept(
                &self.db,
                &name,
                &concept.display_name,
                &concept.description,
                &concept.domain,
            )
            .await?;

            // Embed UUIDs with backtick quoting — safe because UUIDs only contain [0-9a-f-].
            let menciona_sql = format!(
                "RELATE insight:`{insight_id}`->menciona->concept:`{concept_id}` SET relevance_weight = $weight"
            );
            self.db
                .query(&menciona_sql)
                .bind(("weight", concept.relevance_weight))
                .await
                .map_err(|e| RepoError::Internal(e.to_string()))?;

            for related in &concept.related_concepts {
                let related_name = normalize_concept_name(&related.name);
                let related_id =
                    upsert_concept(&self.db, &related_name, &related.name, "", "").await?;

                let rel_sql = format!(
                    "RELATE concept:`{concept_id}`->relacionado_a->concept:`{related_id}` SET relation_type = $rel_type, strength = $strength"
                );
                self.db
                    .query(&rel_sql)
                    .bind(("rel_type", related.relation_type.clone()))
                    .bind(("strength", related.strength))
                    .await
                    .map_err(|e| RepoError::Internal(e.to_string()))?;
            }
        }

        Ok(())
    }
}
