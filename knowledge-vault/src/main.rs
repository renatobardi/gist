use knowledge_vault::{adapters, web};

use std::sync::Arc;

use surrealdb::{engine::local::Db, Surreal};
use tracing::info;
use tracing_subscriber::EnvFilter;

use adapters::{
    nats::publisher::NatsPublisher,
    openlib::OpenLibraryClient,
    surreal::{
        login_attempt_repo::SurrealLoginAttemptRepo, schema::run_migrations,
        token_repo::SurrealTokenRepo, user_repo::SurrealUserRepo, work_repo::SurrealWorkRepo,
    },
};
use web::{router::build_router, state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .json()
        .init();

    info!("Starting knowledge-vault");

    let data_dir = std::env::var("KV_DATA_DIR").unwrap_or_else(|_| "data".to_string());
    let port = std::env::var("KV_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8080);
    let jwt_secret = std::env::var("KV_JWT_SECRET").unwrap_or_else(|_| {
        tracing::warn!(
            "KV_JWT_SECRET is not set — using insecure default. Set this variable in production."
        );
        "dev-secret-change-in-production".to_string()
    });

    info!(data_dir = %data_dir, "Opening database");
    let db = open_db(&data_dir).await?;
    info!("Running schema migrations");
    run_migrations(&db).await?;
    info!("Database schema initialized");

    let nats_url =
        std::env::var("KV_NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    info!(nats_url = %nats_url, "Connecting to NATS");
    let message_publisher: Option<Arc<dyn knowledge_vault::ports::messaging::MessagePublisher>> =
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            async_nats::connect(&nats_url),
        )
        .await
        {
            Ok(Ok(client)) => {
                info!("Connected to NATS at {nats_url}");
                Some(Arc::new(NatsPublisher::new(client)))
            }
            Ok(Err(e)) => {
                tracing::warn!("NATS unavailable — POST /api/works will return 500: {e}");
                None
            }
            Err(_) => {
                tracing::warn!("NATS connect timed out after 5s — POST /api/works will return 500");
                None
            }
        };

    let user_repo = Arc::new(SurrealUserRepo::new(db.clone()));
    let login_attempt_repo = Arc::new(SurrealLoginAttemptRepo::new(db.clone()));
    let token_repo = Arc::new(SurrealTokenRepo::new(db.clone()));
    let work_repo = Arc::new(SurrealWorkRepo::new(db));

    info!("Building HTTP client for Open Library");
    let open_library_client: Option<Arc<dyn knowledge_vault::ports::external::OpenLibraryPort>> =
        match OpenLibraryClient::build() {
            Ok(client) => {
                info!("Open Library HTTP client ready");
                Some(Arc::new(client))
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to build Open Library client — title submissions will return 500: {e}"
                );
                None
            }
        };

    let state = AppState {
        user_repo,
        login_attempt_repo,
        token_repo,
        work_repo,
        message_publisher,
        open_library_client,
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
