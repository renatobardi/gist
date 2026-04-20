use knowledge_vault::{adapters, web};

use std::sync::Arc;

use surrealdb::{engine::local::Db, Surreal};
use tracing::info;
use tracing_subscriber::EnvFilter;

use adapters::surreal::{
    login_attempt_repo::SurrealLoginAttemptRepo,
    schema::run_migrations,
    token_repo::SurrealTokenRepo,
    user_repo::SurrealUserRepo,
};
use web::{router::build_router, state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .json()
        .init();

    let data_dir = std::env::var("KV_DATA_DIR").unwrap_or_else(|_| "data".to_string());
    let port = std::env::var("KV_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8080);
    let jwt_secret = std::env::var("KV_JWT_SECRET").unwrap_or_else(|_| {
        tracing::warn!("KV_JWT_SECRET is not set — using insecure default. Set this variable in production.");
        "dev-secret-change-in-production".to_string()
    });

    let db = open_db(&data_dir).await?;
    run_migrations(&db).await?;
    info!("Database schema initialized");

    let user_repo = Arc::new(SurrealUserRepo::new(db.clone()));
    let login_attempt_repo = Arc::new(SurrealLoginAttemptRepo::new(db.clone()));
    let token_repo = Arc::new(SurrealTokenRepo::new(db));
    let state = AppState {
        user_repo,
        login_attempt_repo,
        token_repo,
        jwt_secret,
    };
    let router = build_router(state);

    let addr = format!("0.0.0.0:{port}");
    info!("Listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}

async fn open_db(data_dir: &str) -> Result<Surreal<Db>, surrealdb::Error> {
    let db_path = format!("{data_dir}/knowledge_vault.surrealkv");
    let db = Surreal::new::<surrealdb::engine::local::SurrealKv>(db_path.as_str()).await?;
    db.use_ns("kv").use_db("kv").await?;
    Ok(db)
}
