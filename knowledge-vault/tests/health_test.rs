use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::TestServer;
use serde_json::Value;
use surrealdb::{engine::local::Mem, Surreal};

use knowledge_vault::{
    adapters::surreal::{
        concept_repo::SurrealConceptRepo, graph_read_repo::SurrealGraphReadRepo,
        graph_write_repo::SurrealGraphWriteRepo, insight_repo::SurrealInsightRepo,
        login_attempt_repo::SurrealLoginAttemptRepo, schema::run_migrations,
        token_repo::SurrealTokenRepo, user_repo::SurrealUserRepo, work_repo::SurrealWorkRepo,
    },
    web::{router::build_router, state::AppState, ws_broadcaster::WsBroadcaster},
};

async fn make_test_server() -> TestServer {
    let db: Surreal<surrealdb::engine::local::Db> = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("kv_test").use_db("kv_test").await.unwrap();
    run_migrations(&db).await.unwrap();

    let user_repo = Arc::new(SurrealUserRepo::new(db.clone()));
    let login_attempt_repo = Arc::new(SurrealLoginAttemptRepo::new(db.clone()));
    let token_repo = Arc::new(SurrealTokenRepo::new(db.clone()));
    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let insight_repo = Arc::new(SurrealInsightRepo::new(db.clone()));
    let concept_repo = Arc::new(SurrealConceptRepo::new(db.clone()));
    let graph_write_repo = Arc::new(SurrealGraphWriteRepo::new(db.clone()));
    let graph_read_repo = Arc::new(SurrealGraphReadRepo::new(db.clone()));
    let state = AppState {
        db: Arc::new(db),
        user_repo,
        login_attempt_repo,
        token_repo,
        work_repo,
        insight_repo,
        concept_repo,
        graph_write_repo,
        graph_read_repo,
        message_publisher: None,
        open_library_client: None,
        google_books_client: None,
        ws_broadcaster: WsBroadcaster::new(),
        jwt_secret: "test-secret".to_string(),
    };

    TestServer::new(build_router(state)).unwrap()
}

#[tokio::test]
async fn health_returns_200_when_db_connected() {
    let server = make_test_server().await;

    let resp = server.get("/health").await;

    resp.assert_status(StatusCode::OK);
    let body: Value = resp.json();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["db"], "connected");
    assert!(body["version"].is_string());
    assert!(!body["version"].as_str().unwrap().is_empty());
}
