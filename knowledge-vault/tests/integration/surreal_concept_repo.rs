use std::sync::Arc;

use serde::Deserialize;
use surrealdb::{engine::local::Mem, Surreal};

use knowledge_vault::{
    adapters::surreal::{
        concept_repo::SurrealConceptRepo,
        insight_repo::SurrealInsightRepo,
        schema::run_migrations,
        work_repo::SurrealWorkRepo,
    },
    ports::repository::{ConceptRepo, InsightRepo, WorkRepo},
};

async fn make_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db: Surreal<surrealdb::engine::local::Db> = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("kv_test").use_db("kv_test").await.unwrap();
    run_migrations(&db).await.unwrap();
    db
}

// ─── ConceptRepo ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn upsert_new_concept_creates_record() {
    let db = make_db().await;
    let repo = Arc::new(SurrealConceptRepo::new(db));

    let concept = repo
        .upsert("Clean Code", "Practices for readable code", "Software Engineering")
        .await
        .unwrap();

    assert!(!concept.id.is_empty());
    assert_eq!(concept.name, "clean code"); // normalized
    assert_eq!(concept.display_name, "Clean Code");
    assert_eq!(concept.description, "Practices for readable code");
    assert_eq!(concept.domain, "Software Engineering");
}

#[tokio::test]
async fn upsert_duplicate_returns_existing_without_creating() {
    let db = make_db().await;
    let repo = Arc::new(SurrealConceptRepo::new(db));

    let first = repo
        .upsert("Clean Code", "First description", "Software Engineering")
        .await
        .unwrap();

    // Same normalized name, different display_name/description — should return first record.
    let second = repo
        .upsert("clean code", "Different description", "Other Domain")
        .await
        .unwrap();

    assert_eq!(first.id, second.id);
    // First-write wins: description must not be overwritten.
    assert_eq!(second.description, "First description");
    assert_eq!(second.display_name, "Clean Code");
}

#[tokio::test]
async fn find_by_name_returns_some_for_existing_concept() {
    let db = make_db().await;
    let repo = Arc::new(SurrealConceptRepo::new(db));

    let created = repo
        .upsert("SOLID Principles", "Object-oriented design guidelines", "Software Engineering")
        .await
        .unwrap();

    let found = repo.find_by_name("solid principles").await.unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.id, created.id);
}

#[tokio::test]
async fn find_by_name_returns_none_for_unknown_concept() {
    let db = make_db().await;
    let repo = Arc::new(SurrealConceptRepo::new(db));

    let found = repo.find_by_name("nonexistent concept").await.unwrap();
    assert!(found.is_none());
}

// ─── InsightRepo ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn create_insight_persists_record() {
    let db = make_db().await;
    let repo = Arc::new(SurrealInsightRepo::new(db));

    let key_points = vec!["Point A".to_string(), "Point B".to_string()];
    let insight = repo
        .create("A summary of the book.", key_points.clone(), r#"{"raw":"json"}"#)
        .await
        .unwrap();

    assert!(!insight.id.is_empty());
    assert_eq!(insight.summary, "A summary of the book.");
    assert_eq!(insight.key_points, key_points);
    assert_eq!(insight.raw_gemini_response, r#"{"raw":"json"}"#);
}

// ─── Edge creation ───────────────────────────────────────────────────────────

#[tokio::test]
async fn create_interpreta_edge_links_work_to_insight() {
    let db = make_db().await;
    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let insight_repo = Arc::new(SurrealInsightRepo::new(db.clone()));

    let work = work_repo.create_work("9780132350884").await.unwrap();
    let insight = insight_repo
        .create("Summary", vec!["key point".to_string()], "{}")
        .await
        .unwrap();

    insight_repo
        .create_interpreta_edge(&work.id, &insight.id)
        .await
        .unwrap();

    #[derive(Deserialize)]
    struct Edge { #[allow(dead_code)] id: Option<surrealdb::sql::Thing> }
    let mut result = db.query("SELECT * FROM interpreta").await.unwrap();
    let edges: Vec<Edge> = result.take(0).unwrap();
    assert_eq!(edges.len(), 1, "expected one interpreta edge");
}

#[tokio::test]
async fn create_menciona_edge_links_insight_to_concept_with_weight() {
    let db = make_db().await;
    let concept_repo = Arc::new(SurrealConceptRepo::new(db.clone()));
    let insight_repo = Arc::new(SurrealInsightRepo::new(db.clone()));

    let insight = insight_repo
        .create("Summary", vec![], "{}")
        .await
        .unwrap();
    let concept = concept_repo
        .upsert("Refactoring", "Improving code structure", "Software Engineering")
        .await
        .unwrap();

    concept_repo
        .create_menciona_edge(&insight.id, &concept.id, 0.85)
        .await
        .unwrap();

    #[derive(Deserialize)]
    struct MencianaEdge { relevance_weight: f64 }
    let mut result = db.query("SELECT relevance_weight FROM menciona").await.unwrap();
    let edges: Vec<MencianaEdge> = result.take(0).unwrap();
    assert_eq!(edges.len(), 1);
    assert!((edges[0].relevance_weight - 0.85).abs() < 1e-9, "relevance_weight mismatch");
}

#[tokio::test]
async fn create_relacionado_a_edge_links_concepts_with_type_and_strength() {
    let db = make_db().await;
    let concept_repo = Arc::new(SurrealConceptRepo::new(db.clone()));

    let concept_a = concept_repo
        .upsert("Clean Code", "Readable code practices", "Software Engineering")
        .await
        .unwrap();
    let concept_b = concept_repo
        .upsert("Refactoring", "Structural code improvement", "Software Engineering")
        .await
        .unwrap();

    concept_repo
        .create_relacionado_a_edge(&concept_a.id, &concept_b.id, "enables", 0.75)
        .await
        .unwrap();

    #[derive(Deserialize)]
    struct RelacionadoAEdge { relation_type: String, strength: f64 }
    let mut result = db.query("SELECT relation_type, strength FROM relacionado_a").await.unwrap();
    let edges: Vec<RelacionadoAEdge> = result.take(0).unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].relation_type, "enables");
    assert!((edges[0].strength - 0.75).abs() < 1e-9, "strength mismatch");
}

#[tokio::test]
async fn upsert_with_mixed_case_display_name_normalizes_correctly() {
    let db = make_db().await;
    let repo = Arc::new(SurrealConceptRepo::new(db));

    let concept = repo
        .upsert("  SOLID Principles  ", "Design principles", "OOP")
        .await
        .unwrap();

    assert_eq!(concept.name, "solid principles");
    assert_eq!(concept.display_name, "SOLID Principles");
}
