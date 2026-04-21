use std::sync::Arc;

use async_trait::async_trait;
use axum::http::StatusCode;
use axum_test::TestServer;
use serde_json::{json, Value};
use surrealdb::{engine::local::Mem, Surreal};

use knowledge_vault::{
    adapters::surreal::{
        concept_repo::SurrealConceptRepo, graph_write_repo::SurrealGraphWriteRepo,
        insight_repo::SurrealInsightRepo, login_attempt_repo::SurrealLoginAttemptRepo,
        schema::run_migrations, token_repo::SurrealTokenRepo, user_repo::SurrealUserRepo,
        work_repo::SurrealWorkRepo,
    },
    ports::{
        external::{BookMetadata, ExternalError, OpenLibraryBook, OpenLibraryPort},
        messaging::MessagePublisher,
    },
    web::{router::build_router, state::AppState, ws_broadcaster::WsBroadcaster},
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

    async fn fetch_by_isbn(&self, _isbn: &str) -> Result<BookMetadata, ExternalError> {
        Err(ExternalError::Permanent(
            "not used in title tests".to_string(),
        ))
    }
}

struct ErrorOpenLibraryClient;

#[async_trait]
impl OpenLibraryPort for ErrorOpenLibraryClient {
    async fn search_by_title(&self, _title: &str) -> Result<Option<OpenLibraryBook>, String> {
        Err("Open Library returned status 503 Service Unavailable".to_string())
    }

    async fn fetch_by_isbn(&self, _isbn: &str) -> Result<BookMetadata, ExternalError> {
        Err(ExternalError::Permanent(
            "not used in title tests".to_string(),
        ))
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
    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let insight_repo = Arc::new(SurrealInsightRepo::new(db.clone()));
    let concept_repo = Arc::new(SurrealConceptRepo::new(db.clone()));
    let graph_write_repo = Arc::new(SurrealGraphWriteRepo::new(db));
    let state = AppState {
        user_repo,
        login_attempt_repo,
        token_repo,
        work_repo,
        insight_repo,
        concept_repo,
        graph_write_repo,
        message_publisher: Some(Arc::new(NoopPublisher)),
        open_library_client: None,
        ws_broadcaster: WsBroadcaster::new(),
        jwt_secret: "test-secret".to_string(),
    };

    TestServer::new(build_router(state)).unwrap()
}

async fn make_test_server_no_nats() -> TestServer {
    let db = make_db().await;

    let user_repo = Arc::new(SurrealUserRepo::new(db.clone()));
    let login_attempt_repo = Arc::new(SurrealLoginAttemptRepo::new(db.clone()));
    let token_repo = Arc::new(SurrealTokenRepo::new(db.clone()));
    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let insight_repo = Arc::new(SurrealInsightRepo::new(db.clone()));
    let concept_repo = Arc::new(SurrealConceptRepo::new(db.clone()));
    let graph_write_repo = Arc::new(SurrealGraphWriteRepo::new(db));
    let state = AppState {
        user_repo,
        login_attempt_repo,
        token_repo,
        work_repo,
        insight_repo,
        concept_repo,
        graph_write_repo,
        message_publisher: None,
        open_library_client: None,
        ws_broadcaster: WsBroadcaster::new(),
        jwt_secret: "test-secret".to_string(),
    };

    TestServer::new(build_router(state)).unwrap()
}

async fn make_test_server_with_mock_ol(ol_result: Option<OpenLibraryBook>) -> TestServer {
    let db = make_db().await;

    let user_repo = Arc::new(SurrealUserRepo::new(db.clone()));
    let login_attempt_repo = Arc::new(SurrealLoginAttemptRepo::new(db.clone()));
    let token_repo = Arc::new(SurrealTokenRepo::new(db.clone()));
    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let insight_repo = Arc::new(SurrealInsightRepo::new(db.clone()));
    let concept_repo = Arc::new(SurrealConceptRepo::new(db.clone()));
    let graph_write_repo = Arc::new(SurrealGraphWriteRepo::new(db));
    let state = AppState {
        user_repo,
        login_attempt_repo,
        token_repo,
        work_repo,
        insight_repo,
        concept_repo,
        graph_write_repo,
        message_publisher: Some(Arc::new(NoopPublisher)),
        open_library_client: Some(Arc::new(MockOpenLibraryClient { result: ol_result })),
        ws_broadcaster: WsBroadcaster::new(),
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

// POST /api/works — valid ISBN returns 202 with work_id
#[tokio::test]
async fn post_works_valid_isbn_returns_202() {
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

// POST /api/works — duplicate ISBN returns 409
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

    let second = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "9780132350884", "identifier_type": "isbn"}))
        .await;

    second.assert_status(StatusCode::CONFLICT);
    let body: Value = second.json();
    assert_eq!(body["work_id"], first_id);
    assert_eq!(body["error"], "duplicate");
}

// POST /api/works — invalid ISBN returns 422
#[tokio::test]
async fn post_works_invalid_isbn_returns_422() {
    let server = make_test_server_with_nats().await;
    let jwt = setup_and_login(&server).await;

    let resp = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "not-an-isbn", "identifier_type": "isbn"}))
        .await;

    resp.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = resp.json();
    assert_eq!(body["error"], "invalid_isbn");
}

// POST /api/works — without auth returns 401
#[tokio::test]
async fn post_works_without_auth_returns_401() {
    let server = make_test_server_with_nats().await;

    let resp = server
        .post("/api/works")
        .json(&json!({"identifier": "9780132350884", "identifier_type": "isbn"}))
        .await;

    resp.assert_status(StatusCode::UNAUTHORIZED);
}

// POST /api/works — without NATS returns 500 messaging_unavailable
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
    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let insight_repo = Arc::new(SurrealInsightRepo::new(db.clone()));
    let concept_repo = Arc::new(SurrealConceptRepo::new(db.clone()));
    let graph_write_repo = Arc::new(SurrealGraphWriteRepo::new(db));
    let state = AppState {
        user_repo,
        login_attempt_repo,
        token_repo,
        work_repo,
        insight_repo,
        concept_repo,
        graph_write_repo,
        message_publisher: Some(Arc::new(NoopPublisher)),
        open_library_client: Some(Arc::new(ErrorOpenLibraryClient)),
        ws_broadcaster: WsBroadcaster::new(),
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

// GET /api/works — empty list when no works submitted
#[tokio::test]
async fn get_works_returns_empty_list_when_no_works() {
    let server = make_test_server_with_nats().await;
    let jwt = setup_and_login(&server).await;

    let resp = server
        .get("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;

    resp.assert_status(StatusCode::OK);
    let body: Value = resp.json();
    assert!(body.is_array());
    assert_eq!(body.as_array().unwrap().len(), 0);
}

// GET /api/works — returns work after submission
#[tokio::test]
async fn get_works_returns_submitted_work() {
    let server = make_test_server_with_nats().await;
    let jwt = setup_and_login(&server).await;

    let post_resp = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "9780132350884", "identifier_type": "isbn"}))
        .await;
    post_resp.assert_status(StatusCode::ACCEPTED);
    let work_id = post_resp.json::<Value>()["work_id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = server
        .get("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;

    resp.assert_status(StatusCode::OK);
    let body: Value = resp.json();
    let works = body.as_array().unwrap();
    assert_eq!(works.len(), 1);
    assert_eq!(works[0]["id"], work_id);
    assert_eq!(works[0]["status"], "pending");
    assert_eq!(works[0]["isbn"], "9780132350884");
}

// GET /api/works — without auth → 401
#[tokio::test]
async fn get_works_without_auth_returns_401() {
    let server = make_test_server_with_nats().await;

    let resp = server.get("/api/works").await;

    resp.assert_status(StatusCode::UNAUTHORIZED);
}

// GET /api/works/{id} — returns work for known id
#[tokio::test]
async fn get_work_by_id_returns_work() {
    let server = make_test_server_with_nats().await;
    let jwt = setup_and_login(&server).await;

    let post_resp = server
        .post("/api/works")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"identifier": "9780132350884", "identifier_type": "isbn"}))
        .await;
    post_resp.assert_status(StatusCode::ACCEPTED);
    let work_id = post_resp.json::<Value>()["work_id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = server
        .get(&format!("/api/works/{work_id}"))
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;

    resp.assert_status(StatusCode::OK);
    let body: Value = resp.json();
    assert_eq!(body["id"], work_id);
    assert_eq!(body["status"], "pending");
}

// GET /api/works/{id} — 404 for unknown id
#[tokio::test]
async fn get_work_by_id_returns_404_for_unknown_id() {
    let server = make_test_server_with_nats().await;
    let jwt = setup_and_login(&server).await;

    let resp = server
        .get("/api/works/00000000-0000-0000-0000-000000000000")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;

    resp.assert_status(StatusCode::NOT_FOUND);
    let body: Value = resp.json();
    assert_eq!(body["error"], "not_found");
}

// GET /api/works/{id} — without auth → 401
#[tokio::test]
async fn get_work_by_id_without_auth_returns_401() {
    let server = make_test_server_with_nats().await;

    let resp = server
        .get("/api/works/00000000-0000-0000-0000-000000000000")
        .await;

    resp.assert_status(StatusCode::UNAUTHORIZED);
}
