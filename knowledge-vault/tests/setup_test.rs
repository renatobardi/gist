use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::TestServer;
use serde_json::{json, Value};
use surrealdb::{engine::local::Mem, Surreal};

use knowledge_vault::{
    adapters::surreal::{
        login_attempt_repo::SurrealLoginAttemptRepo, schema::run_migrations,
        user_repo::SurrealUserRepo,
    },
    web::{router::build_router, state::AppState},
};

async fn make_test_server() -> TestServer {
    let db: Surreal<surrealdb::engine::local::Db> = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("kv_test").use_db("kv_test").await.unwrap();
    run_migrations(&db).await.unwrap();

    let user_repo = Arc::new(SurrealUserRepo::new(db.clone()));
    let login_attempt_repo = Arc::new(SurrealLoginAttemptRepo::new(db));
    let state = AppState {
        user_repo,
        login_attempt_repo,
        jwt_secret: "test-secret".to_string(),
    };
    let router = build_router(state);

    TestServer::new(router).unwrap()
}

// RED: GET /api/setup returns first_run=true when no users exist
#[tokio::test]
async fn get_setup_returns_first_run_true_when_no_users() {
    let server = make_test_server().await;
    let response = server.get("/api/setup").await;
    response.assert_status(StatusCode::OK);
    let body: Value = response.json();
    assert_eq!(body["first_run"], true);
}

// RED: POST /api/setup with valid credentials creates user and returns 201
#[tokio::test]
async fn post_setup_creates_admin_and_returns_201() {
    let server = make_test_server().await;
    let response = server
        .post("/api/setup")
        .json(&json!({
            "email": "admin@example.com",
            "password": "validpassword1"
        }))
        .await;
    response.assert_status(StatusCode::CREATED);
    let body: Value = response.json();
    assert!(body["user_id"].is_string());
    assert!(!body["user_id"].as_str().unwrap().is_empty());
}

// RED: second POST /api/setup returns 409
#[tokio::test]
async fn post_setup_second_time_returns_409() {
    let server = make_test_server().await;

    // First setup
    server
        .post("/api/setup")
        .json(&json!({
            "email": "admin@example.com",
            "password": "validpassword1"
        }))
        .await
        .assert_status(StatusCode::CREATED);

    // Second setup attempt
    let response = server
        .post("/api/setup")
        .json(&json!({
            "email": "other@example.com",
            "password": "validpassword1"
        }))
        .await;
    response.assert_status(StatusCode::CONFLICT);
    let body: Value = response.json();
    assert_eq!(body["error"], "already_configured");
}

// RED: GET /setup redirects to /login when a user exists
#[tokio::test]
async fn get_setup_page_redirects_to_login_when_user_exists() {
    let server = make_test_server().await;

    // Create admin first
    server
        .post("/api/setup")
        .json(&json!({
            "email": "admin@example.com",
            "password": "validpassword1"
        }))
        .await
        .assert_status(StatusCode::CREATED);

    // Now GET /setup should redirect to /login
    let response = server.get("/setup").await;
    response.assert_status(StatusCode::SEE_OTHER);
    assert_eq!(response.headers().get("location").unwrap(), "/login");
}

// RED: POST /api/setup with short password returns 422
#[tokio::test]
async fn post_setup_short_password_returns_422() {
    let server = make_test_server().await;
    let response = server
        .post("/api/setup")
        .json(&json!({
            "email": "admin@example.com",
            "password": "short"
        }))
        .await;
    response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = response.json();
    assert_eq!(body["error"], "invalid_password");
}

// RED: POST /api/setup with invalid email returns 422
#[tokio::test]
async fn post_setup_invalid_email_returns_422() {
    let server = make_test_server().await;
    let response = server
        .post("/api/setup")
        .json(&json!({
            "email": "not-an-email",
            "password": "validpassword1"
        }))
        .await;
    response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = response.json();
    assert_eq!(body["error"], "invalid_email");
}

// RED: root / redirects to /setup on first run
#[tokio::test]
async fn root_redirects_to_setup_on_first_run() {
    let server = make_test_server().await;
    let response = server.get("/").await;
    response.assert_status(StatusCode::SEE_OTHER);
    assert_eq!(response.headers().get("location").unwrap(), "/setup");
}
