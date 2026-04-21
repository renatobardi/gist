/// Integration tests for the worker pipeline logic (S02-09).
///
/// Validates the transactional graph write using in-process SurrealKV (Mem engine)
/// and stub implementations of the external ports.
use std::sync::Arc;

use serde::Deserialize;
use surrealdb::{engine::local::Mem, Surreal};

use knowledge_vault::{
    adapters::surreal::{
        graph_write_repo::SurrealGraphWriteRepo, schema::run_migrations, work_repo::SurrealWorkRepo,
    },
    domain::insight::{ExtractedConcept, GeminiResponse, RelatedConceptRef},
    ports::repository::{GraphWriteRepo, WorkRepo},
};

// ---- Helper ----

async fn make_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db: surrealdb::Surreal<surrealdb::engine::local::Db> =
        Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("kv_test").use_db("kv_test").await.unwrap();
    run_migrations(&db).await.unwrap();
    db
}

fn stub_gemini_response() -> GeminiResponse {
    GeminiResponse {
        summary: "A book about writing clean, maintainable code.".to_string(),
        key_points: vec![
            "Meaningful names".to_string(),
            "Small functions".to_string(),
        ],
        concepts: vec![ExtractedConcept {
            name: "clean code".to_string(),
            display_name: "Clean Code".to_string(),
            description: "Code that is easy to read and maintain.".to_string(),
            domain: "Software Engineering".to_string(),
            relevance_weight: 0.9,
            related_concepts: vec![],
        }],
    }
}

// ---- Tests ----

/// Happy path: write_graph_transaction commits insight, edges, concepts, and sets work = done.
#[tokio::test]
async fn graph_write_transaction_commits_all_data_atomically() {
    let db = make_db().await;

    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let graph_write_repo = Arc::new(SurrealGraphWriteRepo::new(db.clone()));

    let work = work_repo.create_work("9780132350884").await.unwrap();
    assert_eq!(work.status, "pending");

    work_repo
        .update_status(&work.id, "processing", None)
        .await
        .unwrap();

    let gemini_resp = stub_gemini_response();

    graph_write_repo
        .write_graph_transaction(&work.id, &gemini_resp)
        .await
        .unwrap();

    // Work status must be "done" after the transaction
    let updated = work_repo.find_by_id(&work.id).await.unwrap().unwrap();
    assert_eq!(updated.status, "done");

    // Insight node must exist
    #[derive(Deserialize)]
    struct InsightRow {
        summary: String,
    }
    let mut r = db.query("SELECT summary FROM insight").await.unwrap();
    let insights: Vec<InsightRow> = r.take(0).unwrap();
    assert_eq!(insights.len(), 1);
    assert_eq!(insights[0].summary, gemini_resp.summary);

    // interpreta edge must exist
    #[derive(Deserialize)]
    struct EdgeRow {
        #[allow(dead_code)]
        id: Option<surrealdb::sql::Thing>,
    }
    let mut r = db.query("SELECT id FROM interpreta").await.unwrap();
    let edges: Vec<EdgeRow> = r.take(0).unwrap();
    assert_eq!(edges.len(), 1, "interpreta edge missing");

    // Concept node must exist
    #[derive(Deserialize)]
    struct ConceptRow {
        name: String,
    }
    let mut r = db.query("SELECT name FROM concept").await.unwrap();
    let concepts: Vec<ConceptRow> = r.take(0).unwrap();
    assert_eq!(concepts.len(), 1);
    assert_eq!(concepts[0].name, "clean code");

    // menciona edge must exist with correct weight
    #[derive(Deserialize)]
    struct MencianaRow {
        relevance_weight: f64,
    }
    let mut r = db
        .query("SELECT relevance_weight FROM menciona")
        .await
        .unwrap();
    let menciona: Vec<MencianaRow> = r.take(0).unwrap();
    assert_eq!(menciona.len(), 1);
    assert!((menciona[0].relevance_weight - 0.9).abs() < 1e-9);
}

/// The transaction writes relacionado_a edges for related concepts.
#[tokio::test]
async fn graph_write_transaction_creates_relacionado_a_edges() {
    let db = make_db().await;

    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let graph_write_repo = Arc::new(SurrealGraphWriteRepo::new(db.clone()));

    let work = work_repo.create_work("9780132350884").await.unwrap();

    let gemini_resp = GeminiResponse {
        summary: "Summary".to_string(),
        key_points: vec![],
        concepts: vec![ExtractedConcept {
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
        }],
    };

    graph_write_repo
        .write_graph_transaction(&work.id, &gemini_resp)
        .await
        .unwrap();

    #[derive(Deserialize)]
    struct RelEdge {
        relation_type: String,
        strength: f64,
    }
    let mut r = db
        .query("SELECT relation_type, strength FROM relacionado_a")
        .await
        .unwrap();
    let edges: Vec<RelEdge> = r.take(0).unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].relation_type, "enables");
    assert!((edges[0].strength - 0.75).abs() < 1e-9);
}

/// Repeated calls for the same work are idempotent (insight idempotency is guaranteed by
/// InsightRepo; here we verify the transaction itself succeeds twice without duplicate insights).
#[tokio::test]
async fn graph_write_transaction_is_idempotent_for_same_work() {
    let db = make_db().await;

    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let graph_write_repo = Arc::new(SurrealGraphWriteRepo::new(db.clone()));

    let work = work_repo.create_work("9780132350884").await.unwrap();
    let gemini_resp = stub_gemini_response();

    // First call
    graph_write_repo
        .write_graph_transaction(&work.id, &gemini_resp)
        .await
        .unwrap();

    // Second call for the same work — must not panic or produce duplicates
    let result = graph_write_repo
        .write_graph_transaction(&work.id, &gemini_resp)
        .await;
    assert!(result.is_ok(), "second call should not error: {result:?}");

    // Only one insight node should exist
    #[derive(Deserialize)]
    struct Count {
        count: u64,
    }
    let mut r = db
        .query("SELECT count() AS count FROM insight GROUP ALL")
        .await
        .unwrap();
    let counts: Vec<Count> = r.take(0).unwrap();
    assert_eq!(counts[0].count, 1, "duplicate insight created");

    // Only one menciona edge should exist (idempotent re-run must not duplicate edges)
    let mut r = db
        .query("SELECT count() AS count FROM menciona GROUP ALL")
        .await
        .unwrap();
    let counts: Vec<Count> = r.take(0).unwrap();
    assert_eq!(
        counts[0].count, 1,
        "duplicate menciona edge created on second call"
    );

    // Only one concept node should exist
    let mut r = db
        .query("SELECT count() AS count FROM concept GROUP ALL")
        .await
        .unwrap();
    let counts: Vec<Count> = r.take(0).unwrap();
    assert_eq!(
        counts[0].count, 1,
        "duplicate concept created on second call"
    );
}

/// Marks a work as failed when a permanent error occurs (no transaction involved — just status).
#[tokio::test]
async fn worker_pipeline_marks_work_failed_on_permanent_error() {
    let db = make_db().await;
    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));

    let work = work_repo.create_work("9780132350884").await.unwrap();

    work_repo
        .update_status(&work.id, "failed", Some("ISBN not found in OpenLibrary"))
        .await
        .unwrap();

    let updated = work_repo.find_by_id(&work.id).await.unwrap();
    assert!(updated.is_some());
    assert_eq!(updated.unwrap().status, "failed");
}

#[tokio::test]
async fn find_by_id_returns_correct_work() {
    let db = make_db().await;
    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));

    let work = work_repo.create_work("9780132350884").await.unwrap();
    let found = work_repo.find_by_id(&work.id).await.unwrap();

    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.id, work.id);
    assert_eq!(found.isbn.as_deref(), Some("9780132350884"));
}
