use async_trait::async_trait;
use serde::Deserialize;
use surrealdb::{engine::local::Db, sql::Thing, Surreal};

use crate::domain::concept::{
    Concept, ConceptBook, ConceptDetail, GraphData, GraphEdge, GraphNode, RelatedConceptSummary,
};
use crate::ports::repository::{GraphReadRepo, RepoError};

pub struct SurrealGraphReadRepo {
    db: Surreal<Db>,
}

impl SurrealGraphReadRepo {
    pub fn new(db: Surreal<Db>) -> Self {
        Self { db }
    }
}

fn thing_to_uuid(t: Thing) -> String {
    t.id.to_string().trim_matches('`').to_string()
}

#[derive(Deserialize)]
struct ConceptRow {
    id: Option<Thing>,
    name: String,
    display_name: String,
    description: Option<String>,
    domain: String,
}

#[derive(Deserialize)]
struct EdgeRow {
    source: Option<Thing>,
    target: Option<Thing>,
    relation_type: String,
    strength: f64,
}

#[derive(Deserialize)]
struct InsightIdRow {
    insight_id: Option<Thing>,
}

#[derive(Deserialize)]
struct WorkRow {
    work_id: Option<Thing>,
    title: String,
    author: String,
}

#[derive(Deserialize)]
struct RelatedRow {
    concept_id: Option<Thing>,
    display_name: String,
    domain: String,
    relation_type: String,
    strength: f64,
}

#[async_trait]
impl GraphReadRepo for SurrealGraphReadRepo {
    async fn get_graph(&self, domains: Option<Vec<String>>) -> Result<GraphData, RepoError> {
        let node_sql = if domains.is_some() {
            "SELECT id, name, display_name, description, domain FROM concept WHERE domain IN $domains"
        } else {
            "SELECT id, name, display_name, description, domain FROM concept"
        };

        let mut q = self.db.query(node_sql);
        if let Some(ref d) = domains {
            q = q.bind(("domains", d.clone()));
        }

        let mut result = q.await.map_err(|e| RepoError::Internal(e.to_string()))?;

        let rows: Vec<ConceptRow> = result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let nodes: Vec<GraphNode> = rows
            .into_iter()
            .filter_map(|r| {
                r.id.map(|t| GraphNode {
                    id: thing_to_uuid(t),
                    name: r.name,
                    display_name: r.display_name,
                    domain: r.domain,
                })
            })
            .collect();

        let node_ids: std::collections::HashSet<String> =
            nodes.iter().map(|n| n.id.clone()).collect();

        let edge_sql =
            "SELECT in AS source, out AS target, relation_type, strength FROM relacionado_a";
        let mut edge_result = self
            .db
            .query(edge_sql)
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let edge_rows: Vec<EdgeRow> = edge_result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let edges: Vec<GraphEdge> = edge_rows
            .into_iter()
            .filter_map(|r| {
                let src = r.source.map(thing_to_uuid)?;
                let tgt = r.target.map(thing_to_uuid)?;
                if !node_ids.is_empty()
                    && domains.is_some()
                    && (!node_ids.contains(&src) || !node_ids.contains(&tgt))
                {
                    return None;
                }
                Some(GraphEdge {
                    source: src,
                    target: tgt,
                    relation_type: r.relation_type,
                    strength: r.strength,
                })
            })
            .collect();

        Ok(GraphData { nodes, edges })
    }

    async fn get_concept_detail(&self, id: &str) -> Result<Option<ConceptDetail>, RepoError> {
        let concept_sql =
            "SELECT id, name, display_name, description, domain FROM concept WHERE id = type::thing('concept', $id) LIMIT 1";
        let mut result = self
            .db
            .query(concept_sql)
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let rows: Vec<ConceptRow> = result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let concept_row = match rows.into_iter().next() {
            Some(r) => r,
            None => return Ok(None),
        };

        let concept_id_str = match &concept_row.id {
            Some(t) => thing_to_uuid(t.clone()),
            None => return Ok(None),
        };

        let concept = Concept {
            id: concept_id_str.clone(),
            name: concept_row.name,
            display_name: concept_row.display_name,
            description: concept_row.description.unwrap_or_default(),
            domain: concept_row.domain,
        };

        // Get insights that mention this concept
        let insight_sql =
            "SELECT in AS insight_id FROM menciona WHERE out = type::thing('concept', $id)";
        let mut insight_result = self
            .db
            .query(insight_sql)
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let insight_rows: Vec<InsightIdRow> = insight_result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let insight_things: Vec<Thing> = insight_rows
            .into_iter()
            .filter_map(|r| r.insight_id)
            .collect();

        // Get works for those insights
        let books = if insight_things.is_empty() {
            vec![]
        } else {
            let work_sql = "SELECT in AS work_id, in.title AS title, in.author AS author FROM interpreta WHERE out IN $insight_ids";
            let mut work_result = self
                .db
                .query(work_sql)
                .bind(("insight_ids", insight_things))
                .await
                .map_err(|e| RepoError::Internal(e.to_string()))?;

            let work_rows: Vec<WorkRow> = work_result
                .take(0)
                .map_err(|e| RepoError::Internal(e.to_string()))?;

            work_rows
                .into_iter()
                .filter_map(|r| {
                    r.work_id.map(|t| ConceptBook {
                        work_id: thing_to_uuid(t),
                        title: r.title,
                        author: r.author,
                    })
                })
                .collect()
        };

        // Get related concepts (outbound edges)
        let related_sql = "SELECT out AS concept_id, out.display_name AS display_name, out.domain AS domain, relation_type, strength FROM relacionado_a WHERE in = type::thing('concept', $id)";
        let mut related_result = self
            .db
            .query(related_sql)
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let related_rows: Vec<RelatedRow> = related_result
            .take(0)
            .map_err(|e| RepoError::Internal(e.to_string()))?;

        let related_concepts: Vec<RelatedConceptSummary> = related_rows
            .into_iter()
            .filter_map(|r| {
                r.concept_id.map(|t| RelatedConceptSummary {
                    id: thing_to_uuid(t),
                    display_name: r.display_name,
                    domain: r.domain,
                    relation_type: r.relation_type,
                    strength: r.strength,
                })
            })
            .collect();

        Ok(Some(ConceptDetail {
            concept,
            books,
            related_concepts,
        }))
    }
}
