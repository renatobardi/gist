use std::sync::Arc;

use async_trait::async_trait;
use axum::http::StatusCode;
use axum_test::TestServer;
use serde_json::json;
use surrealdb::{engine::local::Mem, Surreal};

use knowledge_vault::{
    adapters::surreal::{
        concept_repo::SurrealConceptRepo, graph_read_repo::SurrealGraphReadRepo,
        graph_write_repo::SurrealGraphWriteRepo, insight_repo::SurrealInsightRepo,
        login_attempt_repo::SurrealLoginAttemptRepo, schema::run_migrations,
        token_repo::SurrealTokenRepo, user_repo::SurrealUserRepo, work_repo::SurrealWorkRepo,
    },
    ports::messaging::MessagePublisher,
    web::{router::build_router, state::AppState, ws_broadcaster::WsBroadcaster},
};

struct NoopPublisher;

#[async_trait]
impl MessagePublisher for NoopPublisher {
    async fn publish(&self, _subject: &str, _payload: Vec<u8>) -> Result<(), String> {
        Ok(())
    }
}

async fn make_test_server() -> TestServer {
    let db: Surreal<surrealdb::engine::local::Db> = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("kv_test").use_db("kv_test").await.unwrap();
    run_migrations(&db).await.unwrap();

    let state = AppState {
        db: Arc::new(db.clone()),
        user_repo: Arc::new(SurrealUserRepo::new(db.clone())),
        login_attempt_repo: Arc::new(SurrealLoginAttemptRepo::new(db.clone())),
        token_repo: Arc::new(SurrealTokenRepo::new(db.clone())),
        work_repo: Arc::new(SurrealWorkRepo::new(db.clone())),
        insight_repo: Arc::new(SurrealInsightRepo::new(db.clone())),
        concept_repo: Arc::new(SurrealConceptRepo::new(db.clone())),
        graph_write_repo: Arc::new(SurrealGraphWriteRepo::new(db.clone())),
        graph_read_repo: Arc::new(SurrealGraphReadRepo::new(db)),
        message_publisher: Some(Arc::new(NoopPublisher)),
        open_library_client: None,
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
    resp.json::<serde_json::Value>()["token"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn get_library_authenticated_returns_200() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;
    let res = server
        .get("/")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    assert_eq!(res.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn get_library_returns_html_content_type() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;
    let res = server
        .get("/")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    let ct = res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(ct.contains("text/html"), "expected text/html, got: {ct}");
}

#[tokio::test]
async fn get_library_contains_key_elements() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;
    let res = server
        .get("/")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    let body = res.text();
    assert!(body.contains("Biblioteca"), "missing Biblioteca heading");
    assert!(
        body.contains("Adicionar Livro"),
        "missing Adicionar Livro button"
    );
    assert!(body.contains("/add"), "missing link to /add");
    assert!(
        body.contains("/api/works"),
        "missing API works endpoint reference"
    );
}

#[tokio::test]
async fn get_library_has_status_badge_styles() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;
    let res = server
        .get("/")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    let body = res.text();
    assert!(
        body.contains("badge-pending"),
        "missing pending badge style"
    );
    assert!(
        body.contains("badge-processing"),
        "missing processing badge style"
    );
    assert!(body.contains("badge-done"), "missing done badge style");
    assert!(body.contains("badge-failed"), "missing failed badge style");
}

#[tokio::test]
async fn get_library_has_accessibility_attributes() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;
    let res = server
        .get("/")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    let body = res.text();
    assert!(body.contains("aria-label"), "missing aria-label");
    assert!(body.contains("aria-live"), "missing aria-live");
}

#[tokio::test]
async fn get_library_unauthenticated_redirects_to_login() {
    let server = make_test_server().await;
    // Set up a user so it's not a first-run scenario
    server
        .post("/api/setup")
        .json(&json!({"email": "admin@example.com", "password": "validpassword1"}))
        .await
        .assert_status(StatusCode::CREATED);

    let res = server.get("/").await;
    assert_eq!(res.status_code(), StatusCode::SEE_OTHER);
    let location = res
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/login", "expected redirect to /login");
}

#[tokio::test]
async fn get_library_no_users_redirects_to_setup() {
    let server = make_test_server().await;
    // No setup — no users in the database
    let res = server.get("/").await;
    assert_eq!(res.status_code(), StatusCode::SEE_OTHER);
    let location = res
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/setup", "expected redirect to /setup");
}
