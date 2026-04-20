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
    ports::{
        external::{OpenLibraryBook, OpenLibraryPort},
        messaging::MessagePublisher,
    },
    web::{router::build_router, state::AppState},
};

struct NoopPublisher;

#[async_trait]
impl MessagePublisher for NoopPublisher {
    async fn publish(&self, _subject: &str, _payload: Vec<u8>) -> Result<(), String> {
        Ok(())
    }
}

struct MockOpenLibraryClient {
    result: Option<OpenLibraryBook>,
}

#[async_trait]
impl OpenLibraryPort for MockOpenLibraryClient {
    async fn search_by_title(&self, _title: &str) -> Result<Option<OpenLibraryBook>, String> {
        Ok(self.result.clone())
    }
}

struct ErrorOpenLibraryClient;

#[async_trait]
impl OpenLibraryPort for ErrorOpenLibraryClient {
    async fn search_by_title(&self, _title: &str) -> Result<Option<OpenLibraryBook>, String> {
        Err("Open Library returned status 503 Service Unavailable".to_string())
    }
}

async fn make_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db: Surreal<surrealdb::engine::local::Db> = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("kv_test").use_db("kv_test").await.unwrap();
    run_migrations(&db).await.unwrap();
    db
}

async fn make_test_server_with_nats() -> TestServer {
    let db = make_db().await;

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
        open_library_client: None,
        jwt_secret: "test-secret".to_string(),
    };

    TestServer::new(build_router(state)).unwrap()
}

async fn make_test_server_no_nats() -> TestServer {
    let db = make_db().await;

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
        open_library_client: None,
        jwt_secret: "test-secret".to_string(),
    };

    TestServer::new(build_router(state)).unwrap()
}

async fn make_test_server_with_mock_ol(ol_result: Option<OpenLibraryBook>) -> TestServer {
    let db = make_db().await;

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
        open_library_client: Some(Arc::new(MockOpenLibraryClient { result: ol_result })),
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

// Valid title → 202 with work_id and status=pending
#[tokio::test]
async fn post_works_valid_title_returns_202() {
    let mock_book = OpenLibraryBook {
        open_library_id: "/works/OL123W".to_string(),
        title: "Clean Code".to_string(),
        author: "Robert C. Martin".to_string(),
    };
    let server = make_test_server_with_mock_ol(Some(mock_book)).await;
    let jwt = setup_and_login(&server).await;

    let resp = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "Clean Code", "identifier_type": "title"}))
        .await;

    resp.assert_status(StatusCode::ACCEPTED);
    let body: Value = resp.json();
    assert!(body["work_id"].is_string());
    assert_eq!(body["status"], "pending");
}

// Title not found in Open Library → 422 title_not_found
#[tokio::test]
async fn post_works_title_not_found_returns_422() {
    let server = make_test_server_with_mock_ol(None).await;
    let jwt = setup_and_login(&server).await;

    let resp = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "Nonexistent Book Title XYZ", "identifier_type": "title"}))
        .await;

    resp.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = resp.json();
    assert_eq!(body["error"], "title_not_found");
}

// Duplicate open_library_id → 409
#[tokio::test]
async fn post_works_duplicate_open_library_id_returns_409() {
    let mock_book = OpenLibraryBook {
        open_library_id: "/works/OL456W".to_string(),
        title: "The Pragmatic Programmer".to_string(),
        author: "David Thomas".to_string(),
    };
    let server = make_test_server_with_mock_ol(Some(mock_book)).await;
    let jwt = setup_and_login(&server).await;

    let first = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "The Pragmatic Programmer", "identifier_type": "title"}))
        .await;
    first.assert_status(StatusCode::ACCEPTED);
    let first_id = first.json::<Value>()["work_id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "The Pragmatic Programmer", "identifier_type": "title"}))
        .await;

    resp.assert_status(StatusCode::CONFLICT);
    let body: Value = resp.json();
    assert_eq!(body["work_id"], first_id);
    assert_eq!(body["error"], "duplicate");
}

// Unknown identifier_type → 422
#[tokio::test]
async fn post_works_unknown_identifier_type_returns_422() {
    let server = make_test_server_with_nats().await;
    let jwt = setup_and_login(&server).await;

    let resp = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "some-value", "identifier_type": "barcode"}))
        .await;

    resp.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = resp.json();
    assert_eq!(body["error"], "invalid_identifier_type");
}

// Empty title → 422 invalid_title
#[tokio::test]
async fn post_works_empty_title_returns_422() {
    let server = make_test_server_with_mock_ol(None).await;
    let jwt = setup_and_login(&server).await;

    let resp = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "   ", "identifier_type": "title"}))
        .await;

    resp.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = resp.json();
    assert_eq!(body["error"], "invalid_title");
}

// Open Library service failure → 500 internal_error
#[tokio::test]
async fn post_works_open_library_error_returns_500() {
    let db = {
        let db: surrealdb::Surreal<surrealdb::engine::local::Db> =
            surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
        db.use_ns("kv_test").use_db("kv_test").await.unwrap();
        knowledge_vault::adapters::surreal::schema::run_migrations(&db)
            .await
            .unwrap();
        db
    };
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
        open_library_client: Some(Arc::new(ErrorOpenLibraryClient)),
        jwt_secret: "test-secret".to_string(),
    };
    let server = TestServer::new(build_router(state)).unwrap();
    let jwt = setup_and_login(&server).await;

    let resp = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "Clean Code", "identifier_type": "title"}))
        .await;

    resp.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
    let body: Value = resp.json();
    assert_eq!(body["error"], "internal_error");
}
