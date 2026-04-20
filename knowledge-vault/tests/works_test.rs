use std::sync::Arc;

use async_trait::async_trait;
use axum::http::StatusCode;
use axum_test::TestServer;
use serde_json::{json, Value};
use surrealdb::{engine::local::Mem, Surreal};

use knowledge_vault::{
    adapters::surreal::{
        login_attempt_repo::SurrealLoginAttemptRepo, schema::run_migrations,
        token_repo::SurrealTokenRepo, user_repo::SurrealUserRepo, work_repo::SurrealWorkRepo,
    },
    ports::messaging::MessagePublisher,
    web::{router::build_router, state::AppState},
};

struct NoopPublisher;

#[async_trait]
impl MessagePublisher for NoopPublisher {
    async fn publish(&self, _subject: &str, _payload: Vec<u8>) -> Result<(), String> {
        Ok(())
    }
}

async fn make_test_server_with_nats() -> TestServer {
    let db: Surreal<surrealdb::engine::local::Db> = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("kv_test").use_db("kv_test").await.unwrap();
    run_migrations(&db).await.unwrap();

    let user_repo = Arc::new(SurrealUserRepo::new(db.clone()));
    let login_attempt_repo = Arc::new(SurrealLoginAttemptRepo::new(db.clone()));
    let token_repo = Arc::new(SurrealTokenRepo::new(db.clone()));
    let work_repo = Arc::new(SurrealWorkRepo::new(db));
    let state = AppState {
        user_repo,
        login_attempt_repo,
        token_repo,
        work_repo,
        message_publisher: Some(Arc::new(NoopPublisher)),
        jwt_secret: "test-secret".to_string(),
    };

    TestServer::new(build_router(state)).unwrap()
}

async fn make_test_server_no_nats() -> TestServer {
    let db: Surreal<surrealdb::engine::local::Db> = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("kv_test").use_db("kv_test").await.unwrap();
    run_migrations(&db).await.unwrap();

    let user_repo = Arc::new(SurrealUserRepo::new(db.clone()));
    let login_attempt_repo = Arc::new(SurrealLoginAttemptRepo::new(db.clone()));
    let token_repo = Arc::new(SurrealTokenRepo::new(db.clone()));
    let work_repo = Arc::new(SurrealWorkRepo::new(db));
    let state = AppState {
        user_repo,
        login_attempt_repo,
        token_repo,
        work_repo,
        message_publisher: None,
        jwt_secret: "test-secret".to_string(),
    };

    TestServer::new(build_router(state)).unwrap()
}

async fn setup_and_login(server: &TestServer) -> String {
    server
        .post("/api/setup")
        .json(&json!({"email": "admin@example.com", "password": "validpassword1"}))
        .await
        .assert_status(StatusCode::CREATED);

    let resp = server
        .post("/auth/login")
        .json(&json!({"email": "admin@example.com", "password": "validpassword1"}))
        .await;
    resp.assert_status(StatusCode::OK);
    resp.json::<Value>()["token"].as_str().unwrap().to_string()
}

// Valid ISBN-13 → 202 with work_id and status=pending
#[tokio::test]
async fn post_works_valid_isbn13_returns_202() {
    let server = make_test_server_with_nats().await;
    let jwt = setup_and_login(&server).await;

    let resp = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "9780132350884", "identifier_type": "isbn"}))
        .await;

    resp.assert_status(StatusCode::ACCEPTED);
    let body: Value = resp.json();
    assert!(body["work_id"].is_string());
    assert_eq!(body["status"], "pending");
}

// Duplicate ISBN → 409 with existing work_id
#[tokio::test]
async fn post_works_duplicate_isbn_returns_409() {
    let server = make_test_server_with_nats().await;
    let jwt = setup_and_login(&server).await;

    let first = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "9780132350884", "identifier_type": "isbn"}))
        .await;
    first.assert_status(StatusCode::ACCEPTED);
    let first_id = first.json::<Value>()["work_id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "9780132350884", "identifier_type": "isbn"}))
        .await;

    resp.assert_status(StatusCode::CONFLICT);
    let body: Value = resp.json();
    assert_eq!(body["work_id"], first_id);
    assert_eq!(body["error"], "duplicate");
}

// Invalid ISBN → 422
#[tokio::test]
async fn post_works_invalid_isbn_returns_422() {
    let server = make_test_server_with_nats().await;
    let jwt = setup_and_login(&server).await;

    let resp = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "1234567890123", "identifier_type": "isbn"}))
        .await;

    resp.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = resp.json();
    assert_eq!(body["error"], "invalid_isbn");
    assert!(body["message"].is_string());
}

// Unauthenticated request → 401
#[tokio::test]
async fn post_works_without_auth_returns_401() {
    let server = make_test_server_with_nats().await;

    let resp = server
        .post("/api/works")
        .json(&json!({"identifier": "9780132350884", "identifier_type": "isbn"}))
        .await;

    resp.assert_status(StatusCode::UNAUTHORIZED);
}

// ISBN with hyphens is accepted (normalised before validation)
#[tokio::test]
async fn post_works_isbn_with_hyphens_is_accepted() {
    let server = make_test_server_with_nats().await;
    let jwt = setup_and_login(&server).await;

    let resp = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "978-0-13-235088-4", "identifier_type": "isbn"}))
        .await;

    resp.assert_status(StatusCode::ACCEPTED);
}

// NATS unavailable → 500, and no work record is persisted
#[tokio::test]
async fn post_works_without_nats_returns_500() {
    let server = make_test_server_no_nats().await;
    let jwt = setup_and_login(&server).await;

    let resp = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "9780132350884", "identifier_type": "isbn"}))
        .await;

    resp.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
    let body: Value = resp.json();
    assert_eq!(body["error"], "messaging_unavailable");

    // Verify no work record was created: a retry must still return
    // messaging_unavailable (500), not duplicate (409).
    let retry = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "9780132350884", "identifier_type": "isbn"}))
        .await;
    retry.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
    let retry_body: Value = retry.json();
    assert_eq!(retry_body["error"], "messaging_unavailable");
}
