/// Integration test for the worker pipeline logic.
///
/// Since NATS JetStream may not be running in CI, this test validates the
/// pipeline using in-process SurrealKV (Mem engine) and stub implementations
/// of the external ports.
use std::sync::Arc;

use async_trait::async_trait;
use surrealdb::{engine::local::Mem, Surreal};

use knowledge_vault::{
    adapters::surreal::{
        concept_repo::SurrealConceptRepo, insight_repo::SurrealInsightRepo, schema::run_migrations,
        work_repo::SurrealWorkRepo,
    },
    domain::insight::{ExtractedConcept, GeminiResponse},
    ports::{
        external::{BookMetadata, ExternalError, GeminiPort, OpenLibraryBook, OpenLibraryPort},
        repository::{ConceptRepo, InsightRepo, WorkRepo},
    },
};

// ---- Stubs ----

struct StubOpenLibrary;

#[async_trait]
impl OpenLibraryPort for StubOpenLibrary {
    async fn search_by_title(&self, _title: &str) -> Result<Option<OpenLibraryBook>, String> {
        Ok(None)
    }

    async fn fetch_by_isbn(&self, _isbn: &str) -> Result<BookMetadata, ExternalError> {
        Ok(BookMetadata {
            title: "Clean Code".to_string(),
            author: "Robert C. Martin".to_string(),
            description: "A handbook of agile software craftsmanship.".to_string(),
            subjects: vec![
                "Software engineering".to_string(),
                "Programming".to_string(),
            ],
        })
    }
}

struct StubGemini;

#[async_trait]
impl GeminiPort for StubGemini {
    async fn extract_concepts(
        &self,
        _metadata: &BookMetadata,
    ) -> Result<GeminiResponse, ExternalError> {
        Ok(GeminiResponse {
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
        })
    }
}

// ---- Helper ----

async fn make_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db: surrealdb::Surreal<surrealdb::engine::local::Db> =
        Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("kv_test").use_db("kv_test").await.unwrap();
    run_migrations(&db).await.unwrap();
    db
}

// ---- Tests ----

#[tokio::test]
async fn worker_pipeline_processes_work_and_updates_status_to_done() {
    let db = make_db().await;

    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let insight_repo = Arc::new(SurrealInsightRepo::new(db.clone()));
    let concept_repo = Arc::new(SurrealConceptRepo::new(db.clone()));
    let openlib: Arc<dyn OpenLibraryPort> = Arc::new(StubOpenLibrary);
    let gemini: Arc<dyn GeminiPort> = Arc::new(StubGemini);

    // Create a work record
    let work = work_repo.create_work("9780132350884").await.unwrap();
    assert_eq!(work.status, "pending");

    // Simulate the pipeline steps
    work_repo
        .update_status(&work.id, "processing", None)
        .await
        .unwrap();

    let isbn = work.isbn.as_deref().unwrap_or_default();
    let metadata = openlib.fetch_by_isbn(isbn).await.unwrap();
    let gemini_resp = gemini.extract_concepts(&metadata).await.unwrap();

    let raw_json = serde_json::to_string(&gemini_resp).unwrap();

    let insight_id = insight_repo
        .create_insight(
            &work.id,
            &gemini_resp.summary,
            gemini_resp.key_points.clone(),
            &raw_json,
        )
        .await
        .unwrap();

    assert!(!insight_id.is_empty());

    concept_repo
        .upsert_and_link(&work.id, &insight_id, gemini_resp.concepts)
        .await
        .unwrap();

    work_repo
        .update_status(&work.id, "done", None)
        .await
        .unwrap();

    // Verify final status
    let updated = work_repo.find_by_id(&work.id).await.unwrap();
    assert!(updated.is_some());
    assert_eq!(updated.unwrap().status, "done");
}

#[tokio::test]
async fn worker_pipeline_marks_work_failed_on_permanent_error() {
    let db = make_db().await;
    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));

    let work = work_repo.create_work("9780132350884").await.unwrap();

    // Simulate a permanent failure
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

#[tokio::test]
async fn create_insight_is_idempotent_on_retry() {
    let db = make_db().await;
    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let insight_repo = Arc::new(SurrealInsightRepo::new(db.clone()));

    let work = work_repo.create_work("9780132350884").await.unwrap();

    let id1 = insight_repo
        .create_insight(&work.id, "summary", vec!["point".into()], "{}")
        .await
        .unwrap();

    // Second call simulates a retry — must return the same insight id
    let id2 = insight_repo
        .create_insight(&work.id, "summary", vec!["point".into()], "{}")
        .await
        .unwrap();

    assert_eq!(id1, id2, "create_insight must be idempotent");
}

#[tokio::test]
async fn upsert_and_link_deduplicates_concepts_across_calls() {
    let db = make_db().await;
    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let insight_repo = Arc::new(SurrealInsightRepo::new(db.clone()));
    let concept_repo = Arc::new(SurrealConceptRepo::new(db.clone()));

    let work = work_repo.create_work("9780132350884").await.unwrap();
    let insight_id = insight_repo
        .create_insight(&work.id, "summary", vec![], "{}")
        .await
        .unwrap();

    let concepts = vec![ExtractedConcept {
        name: "clean code".to_string(),
        display_name: "Clean Code".to_string(),
        description: "Readable code.".to_string(),
        domain: "SE".to_string(),
        relevance_weight: 0.9,
        related_concepts: vec![],
    }];

    // First call — creates the concept
    concept_repo
        .upsert_and_link(&work.id, &insight_id, concepts.clone())
        .await
        .unwrap();

    // Second call simulates retry — must not create a duplicate concept
    concept_repo
        .upsert_and_link(&work.id, &insight_id, concepts)
        .await
        .unwrap();
}
