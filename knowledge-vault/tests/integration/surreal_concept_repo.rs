/// Integration tests for concept & edge persistence (S02-08).
///
/// Validates concept deduplication, menciona edges, relacionado_a edges,
/// insight creation, interpreta edge idempotency, and serde defaults.
use std::sync::Arc;

use serde::Deserialize;
use surrealdb::{engine::local::Mem, Surreal};

use knowledge_vault::{
    adapters::surreal::{
        concept_repo::SurrealConceptRepo, insight_repo::SurrealInsightRepo, schema::run_migrations,
        work_repo::SurrealWorkRepo,
    },
    domain::insight::{ExtractedConcept, RelatedConceptRef},
    ports::repository::{ConceptRepo, InsightRepo, WorkRepo},
};

async fn make_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db: Surreal<surrealdb::engine::local::Db> = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("kv_test").use_db("kv_test").await.unwrap();
    run_migrations(&db).await.unwrap();
    db
}

fn clean_code_concept() -> ExtractedConcept {
    ExtractedConcept {
        name: "clean code".to_string(),
        display_name: "Clean Code".to_string(),
        description: "Practices for writing readable, maintainable code.".to_string(),
        domain: "Software Engineering".to_string(),
        relevance_weight: 0.9,
        related_concepts: vec![],
    }
}

// ─── InsightRepo ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn create_insight_persists_and_creates_interpreta_edge() {
    let db = make_db().await;
    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let insight_repo = Arc::new(SurrealInsightRepo::new(db.clone()));

    let work = work_repo.create_work("9780132350884").await.unwrap();

    let insight_id = insight_repo
        .create_insight(
            &work.id,
            "A great book summary.",
            vec!["Point A".to_string()],
            "{}",
        )
        .await
        .unwrap();

    assert!(!insight_id.is_empty());

    // Verify interpreta edge was created
    #[derive(Deserialize)]
    struct Edge {
        #[allow(dead_code)]
        id: Option<surrealdb::sql::Thing>,
    }
    let mut result = db.query("SELECT * FROM interpreta").await.unwrap();
    let edges: Vec<Edge> = result.take(0).unwrap();
    assert_eq!(edges.len(), 1, "expected one interpreta edge");
}

#[tokio::test]
async fn create_insight_is_idempotent_for_same_work() {
    let db = make_db().await;
    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let insight_repo = Arc::new(SurrealInsightRepo::new(db.clone()));

    let work = work_repo.create_work("9780132350884").await.unwrap();

    let id1 = insight_repo
        .create_insight(&work.id, "Summary", vec![], "{}")
        .await
        .unwrap();

    // Second call for the same work — must return the same insight ID
    let id2 = insight_repo
        .create_insight(
            &work.id,
            "Different summary",
            vec!["ignored".to_string()],
            "{}",
        )
        .await
        .unwrap();

    assert_eq!(
        id1, id2,
        "create_insight must be idempotent for the same work"
    );

    // Only one interpreta edge should exist
    #[derive(Deserialize)]
    struct Edge {
        #[allow(dead_code)]
        id: Option<surrealdb::sql::Thing>,
    }
    let mut result = db.query("SELECT * FROM interpreta").await.unwrap();
    let edges: Vec<Edge> = result.take(0).unwrap();
    assert_eq!(
        edges.len(),
        1,
        "idempotent call must not create a second interpreta edge"
    );
}

// ─── ConceptRepo ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn upsert_and_link_creates_concept_and_menciona_edge() {
    let db = make_db().await;
    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let insight_repo = Arc::new(SurrealInsightRepo::new(db.clone()));
    let concept_repo = Arc::new(SurrealConceptRepo::new(db.clone()));

    let work = work_repo.create_work("9780132350884").await.unwrap();
    let insight_id = insight_repo
        .create_insight(&work.id, "Summary", vec![], "{}")
        .await
        .unwrap();

    concept_repo
        .upsert_and_link(&work.id, &insight_id, vec![clean_code_concept()])
        .await
        .unwrap();

    // Verify concept record exists
    #[derive(Deserialize)]
    struct ConceptRow {
        name: String,
        display_name: String,
    }
    let mut result = db
        .query("SELECT name, display_name FROM concept")
        .await
        .unwrap();
    let concepts: Vec<ConceptRow> = result.take(0).unwrap();
    assert_eq!(concepts.len(), 1);
    assert_eq!(concepts[0].name, "clean code");
    assert_eq!(concepts[0].display_name, "Clean Code");

    // Verify menciona edge with relevance_weight
    #[derive(Deserialize)]
    struct MencianaEdge {
        relevance_weight: f64,
    }
    let mut result = db
        .query("SELECT relevance_weight FROM menciona")
        .await
        .unwrap();
    let edges: Vec<MencianaEdge> = result.take(0).unwrap();
    assert_eq!(edges.len(), 1);
    assert!(
        (edges[0].relevance_weight - 0.9).abs() < 1e-9,
        "relevance_weight mismatch"
    );
}

#[tokio::test]
async fn upsert_and_link_deduplicates_concepts_by_normalized_name() {
    let db = make_db().await;
    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let insight_repo = Arc::new(SurrealInsightRepo::new(db.clone()));
    let concept_repo = Arc::new(SurrealConceptRepo::new(db.clone()));

    // First book — creates the concept
    let work1 = work_repo.create_work("9780132350884").await.unwrap();
    let insight1 = insight_repo
        .create_insight(&work1.id, "Summary 1", vec![], "{}")
        .await
        .unwrap();
    concept_repo
        .upsert_and_link(&work1.id, &insight1, vec![clean_code_concept()])
        .await
        .unwrap();

    // Second book — same concept (different casing), must not create a duplicate
    let work2 = work_repo.create_work("9780201633610").await.unwrap();
    let insight2 = insight_repo
        .create_insight(&work2.id, "Summary 2", vec![], "{}")
        .await
        .unwrap();
    let duplicate = ExtractedConcept {
        name: "clean code".to_string(),
        display_name: "CLEAN CODE".to_string(), // different casing
        description: "Another description".to_string(),
        domain: "CS".to_string(),
        relevance_weight: 0.5,
        related_concepts: vec![],
    };
    concept_repo
        .upsert_and_link(&work2.id, &insight2, vec![duplicate])
        .await
        .unwrap();

    // Only one concept record should exist
    #[derive(Deserialize)]
    struct Count {
        count: u64,
    }
    let mut result = db
        .query("SELECT count() AS count FROM concept GROUP ALL")
        .await
        .unwrap();
    let counts: Vec<Count> = result.take(0).unwrap();
    assert_eq!(
        counts[0].count, 1,
        "concept must be deduplicated by normalized name"
    );

    // Two menciona edges (one per insight)
    let mut result2 = db
        .query("SELECT count() AS count FROM menciona GROUP ALL")
        .await
        .unwrap();
    let counts2: Vec<Count> = result2.take(0).unwrap();
    assert_eq!(
        counts2[0].count, 2,
        "each insight should have its own menciona edge to the shared concept"
    );
}

#[tokio::test]
async fn upsert_and_link_creates_relacionado_a_edges() {
    let db = make_db().await;
    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let insight_repo = Arc::new(SurrealInsightRepo::new(db.clone()));
    let concept_repo = Arc::new(SurrealConceptRepo::new(db.clone()));

    let work = work_repo.create_work("9780132350884").await.unwrap();
    let insight_id = insight_repo
        .create_insight(&work.id, "Summary", vec![], "{}")
        .await
        .unwrap();

    let concept_with_relation = ExtractedConcept {
        name: "clean code".to_string(),
        display_name: "Clean Code".to_string(),
        description: "Readable code".to_string(),
        domain: "Software Engineering".to_string(),
        relevance_weight: 0.9,
        related_concepts: vec![RelatedConceptRef {
            name: "Refactoring".to_string(),
            relation_type: "enables".to_string(),
            strength: 0.75,
        }],
    };

    concept_repo
        .upsert_and_link(&work.id, &insight_id, vec![concept_with_relation])
        .await
        .unwrap();

    #[derive(Deserialize)]
    struct RelEdge {
        relation_type: String,
        strength: f64,
    }
    let mut result = db
        .query("SELECT relation_type, strength FROM relacionado_a")
        .await
        .unwrap();
    let edges: Vec<RelEdge> = result.take(0).unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].relation_type, "enables");
    assert!((edges[0].strength - 0.75).abs() < 1e-9, "strength mismatch");
}

#[tokio::test]
async fn related_concept_defaults_applied_when_fields_omitted() {
    // Verifies that serde defaults on RelatedConceptRef produce "related" and 0.5
    // when those fields are absent from the JSON (as Gemini may omit them).
    let json = r#"{"name": "Refactoring"}"#;
    let parsed: RelatedConceptRef = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.relation_type, "related");
    assert!(
        (parsed.strength - 0.5).abs() < 1e-9,
        "default strength must be 0.5"
    );
}
