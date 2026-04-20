use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::TestServer;
use serde_json::{json, Value};
use surrealdb::{engine::local::Mem, Surreal};

use knowledge_vault::{
    adapters::surreal::{
        login_attempt_repo::SurrealLoginAttemptRepo, schema::run_migrations,
        token_repo::SurrealTokenRepo, user_repo::SurrealUserRepo, work_repo::SurrealWorkRepo,
    },
    web::{router::build_router, state::AppState},
};

async fn make_test_server() -> TestServer {
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

async fn create_admin(server: &TestServer) {
    server
        .post("/api/setup")
        .json(&json!({
            "email": "admin@example.com",
            "password": "validpassword1"
        }))
        .await
        .assert_status(StatusCode::CREATED);
}

// Valid credentials return 200 with token in body and Set-Cookie header
#[tokio::test]
async fn login_with_valid_credentials_returns_200_with_token() {
    let server = make_test_server().await;
    create_admin(&server).await;

    let response = server
        .post("/auth/login")
        .json(&json!({
            "email": "admin@example.com",
            "password": "validpassword1"
        }))
        .await;

    response.assert_status(StatusCode::OK);

    let body: Value = response.json();
    assert!(
        body["token"].is_string(),
        "response must contain a token string"
    );
    let token = body["token"].as_str().unwrap();
    assert!(!token.is_empty(), "token must not be empty");

    let cookie = response
        .headers()
        .get("set-cookie")
        .expect("Set-Cookie header must be present");
    let cookie_str = cookie.to_str().unwrap();
    assert!(
        cookie_str.contains("session="),
        "cookie must contain session="
    );
    assert!(
        cookie_str.to_lowercase().contains("httponly"),
        "cookie must be HttpOnly"
    );
    assert!(
        cookie_str.to_lowercase().contains("secure"),
        "cookie must be Secure"
    );
    assert!(
        cookie_str.to_lowercase().contains("samesite=strict"),
        "cookie must be SameSite=Strict"
    );
}

// Wrong password returns 401
#[tokio::test]
async fn login_with_wrong_password_returns_401() {
    let server = make_test_server().await;
    create_admin(&server).await;

    let response = server
        .post("/auth/login")
        .json(&json!({
            "email": "admin@example.com",
            "password": "wrongpassword!"
        }))
        .await;

    response.assert_status(StatusCode::UNAUTHORIZED);
    let body: Value = response.json();
    assert_eq!(body["error"], "invalid_credentials");
}

// Unknown email returns 401
#[tokio::test]
async fn login_with_unknown_email_returns_401() {
    let server = make_test_server().await;
    create_admin(&server).await;

    let response = server
        .post("/auth/login")
        .json(&json!({
            "email": "nobody@example.com",
            "password": "validpassword1"
        }))
        .await;

    response.assert_status(StatusCode::UNAUTHORIZED);
    let body: Value = response.json();
    assert_eq!(body["error"], "invalid_credentials");
}

// After 3 failures for same email, 4th attempt returns 429
#[tokio::test]
async fn login_rate_limited_after_3_failures() {
    let server = make_test_server().await;
    create_admin(&server).await;

    for _ in 0..3 {
        server
            .post("/auth/login")
            .json(&json!({
                "email": "admin@example.com",
                "password": "wrongpassword!"
            }))
            .await
            .assert_status(StatusCode::UNAUTHORIZED);
    }

    let response = server
        .post("/auth/login")
        .json(&json!({
            "email": "admin@example.com",
            "password": "wrongpassword!"
        }))
        .await;

    response.assert_status(StatusCode::TOO_MANY_REQUESTS);
    let body: Value = response.json();
    assert_eq!(body["error"], "rate_limited");
    assert!(body["retry_after_seconds"].as_u64().unwrap_or(0) > 0);
}

// Rate limit applies even with correct password after lockout
#[tokio::test]
async fn login_rate_limited_blocks_even_correct_password() {
    let server = make_test_server().await;
    create_admin(&server).await;

    for _ in 0..3 {
        server
            .post("/auth/login")
            .json(&json!({
                "email": "admin@example.com",
                "password": "wrongpassword!"
            }))
            .await
            .assert_status(StatusCode::UNAUTHORIZED);
    }

    let response = server
        .post("/auth/login")
        .json(&json!({
            "email": "admin@example.com",
            "password": "validpassword1"
        }))
        .await;

    response.assert_status(StatusCode::TOO_MANY_REQUESTS);
    let body: Value = response.json();
    assert_eq!(body["error"], "rate_limited");
}

// Rate limit is per-email: different email is not affected
#[tokio::test]
async fn login_rate_limit_is_per_email() {
    let server = make_test_server().await;
    create_admin(&server).await;

    // Exhaust failures for unknown email
    for _ in 0..3 {
        server
            .post("/auth/login")
            .json(&json!({
                "email": "other@example.com",
                "password": "wrongpassword!"
            }))
            .await
            .assert_status(StatusCode::UNAUTHORIZED);
    }

    // Admin email should still work
    let response = server
        .post("/auth/login")
        .json(&json!({
            "email": "admin@example.com",
            "password": "validpassword1"
        }))
        .await;

    response.assert_status(StatusCode::OK);
}
