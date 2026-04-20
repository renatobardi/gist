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
    web::{router::build_router, state::AppState, ws_broadcaster::WsBroadcaster},
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
    resp.json::<Value>()["token"].as_str().unwrap().to_string()
}

// POST /api/tokens with valid JWT returns 201 with ens_ token shown once
#[tokio::test]
async fn create_token_returns_201_with_ens_prefix() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;

    let resp = server
        .post("/api/tokens")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"name": "my-token"}))
        .await;

    resp.assert_status(StatusCode::CREATED);
    let body: Value = resp.json();
    assert!(body["token_id"].is_string());
    assert!(body["token"].as_str().unwrap().starts_with("ens_"));
    assert_eq!(body["name"], "my-token");
}

// POST /api/tokens without auth returns 401
#[tokio::test]
async fn create_token_without_auth_returns_401() {
    let server = make_test_server().await;
    let _ = setup_and_login(&server).await;

    let resp = server
        .post("/api/tokens")
        .json(&json!({"name": "my-token"}))
        .await;

    resp.assert_status(StatusCode::UNAUTHORIZED);
}

// GET /api/tokens returns the created token (without the raw token value)
#[tokio::test]
async fn list_tokens_returns_created_tokens() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;

    server
        .post("/api/tokens")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"name": "ci-token"}))
        .await
        .assert_status(StatusCode::CREATED);

    let resp = server
        .get("/api/tokens")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;

    resp.assert_status(StatusCode::OK);
    let items: Vec<Value> = resp.json();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "ci-token");
    assert!(items[0]["token_id"].is_string());
    assert!(
        items[0].get("token").is_none(),
        "raw token must not be returned in list"
    );
}

// DELETE /api/tokens/{id} revokes the token → 204
#[tokio::test]
async fn delete_token_returns_204_and_token_no_longer_listed() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;

    let create_resp = server
        .post("/api/tokens")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"name": "temp-token"}))
        .await;
    create_resp.assert_status(StatusCode::CREATED);
    let token_id = create_resp.json::<Value>()["token_id"]
        .as_str()
        .unwrap()
        .to_string();

    let del_resp = server
        .delete(&format!("/api/tokens/{token_id}"))
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    del_resp.assert_status(StatusCode::NO_CONTENT);

    let list: Vec<Value> = server
        .get("/api/tokens")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await
        .json();
    assert!(list.is_empty(), "revoked token must not appear in list");
}

// DELETE /api/tokens/{id} for non-existent id returns 404
#[tokio::test]
async fn delete_nonexistent_token_returns_404() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;

    let resp = server
        .delete("/api/tokens/00000000-0000-0000-0000-000000000000")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;

    resp.assert_status(StatusCode::NOT_FOUND);
}

// PAT can be used to authenticate (used on POST /api/tokens to create a second token)
#[tokio::test]
async fn pat_authenticates_successfully() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;

    let create_resp = server
        .post("/api/tokens")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"name": "api-key"}))
        .await;
    create_resp.assert_status(StatusCode::CREATED);
    let pat = create_resp.json::<Value>()["token"]
        .as_str()
        .unwrap()
        .to_string();

    // Use the PAT to create another token
    let resp = server
        .post("/api/tokens")
        .add_header("Authorization", format!("Bearer {pat}"))
        .json(&json!({"name": "second-token"}))
        .await;

    resp.assert_status(StatusCode::CREATED);
    assert!(resp.json::<Value>()["token"]
        .as_str()
        .unwrap()
        .starts_with("ens_"));
}

// Revoked PAT is rejected with 401 on subsequent use
#[tokio::test]
async fn revoked_pat_is_rejected() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;

    let create_resp = server
        .post("/api/tokens")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"name": "short-lived"}))
        .await;
    create_resp.assert_status(StatusCode::CREATED);
    let body: Value = create_resp.json();
    let pat = body["token"].as_str().unwrap().to_string();
    let token_id = body["token_id"].as_str().unwrap().to_string();

    server
        .delete(&format!("/api/tokens/{token_id}"))
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    let resp = server
        .get("/api/tokens")
        .add_header("Authorization", format!("Bearer {pat}"))
        .await;
    resp.assert_status(StatusCode::UNAUTHORIZED);
}

// POST /api/tokens with empty name returns 422
#[tokio::test]
async fn create_token_with_empty_name_returns_422() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;

    let resp = server
        .post("/api/tokens")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"name": "   "}))
        .await;

    resp.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
}
