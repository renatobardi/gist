use knowledge_vault::{adapters, app, web};

use std::sync::Arc;

use surrealdb::{engine::local::Db, Surreal};
use tracing::info;
use tracing_subscriber::EnvFilter;

use adapters::{
    gemini::GeminiAdapter,
    nats::publisher::NatsPublisher,
    openlib::OpenLibraryAdapter,
    surreal::{
        concept_repo::SurrealConceptRepo,
        insight_repo::SurrealInsightRepo,
        login_attempt_repo::SurrealLoginAttemptRepo,
        schema::run_migrations,
        token_repo::SurrealTokenRepo,
        user_repo::SurrealUserRepo,
        work_repo::SurrealWorkRepo,
    },
};
use app::worker::WorkerService;
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
        tracing::warn!(
            "KV_JWT_SECRET is not set — using insecure default. Set this variable in production."
        );
        "dev-secret-change-in-production".to_string()
    });

    let db = open_db(&data_dir).await?;
    run_migrations(&db).await?;
    info!("Database schema initialized");

    let nats_url =
        std::env::var("KV_NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());

    let nats_client = match async_nats::connect(&nats_url).await {
        Ok(client) => {
            info!("Connected to NATS at {nats_url}");
            Some(client)
        }
        Err(e) => {
            tracing::warn!("NATS unavailable — POST /api/works will return 500: {e}");
            None
        }
    };

    let message_publisher: Option<Arc<dyn knowledge_vault::ports::messaging::MessagePublisher>> =
        nats_client
            .as_ref()
            .map(|c| Arc::new(NatsPublisher::new(c.clone())) as Arc<_>);

    let work_repo: Arc<SurrealWorkRepo> = Arc::new(SurrealWorkRepo::new(db.clone()));
    let concept_repo: Arc<SurrealConceptRepo> = Arc::new(SurrealConceptRepo::new(db.clone()));
    let insight_repo: Arc<SurrealInsightRepo> = Arc::new(SurrealInsightRepo::new(db.clone()));

    // Start worker if NATS and Gemini API key are available
    if let Some(nats) = nats_client {
        match std::env::var("KV_GEMINI_API_KEY") {
            Ok(api_key) => {
                let gemini = Arc::new(GeminiAdapter::new(api_key));
                let open_library = Arc::new(OpenLibraryAdapter::new());
                let worker = WorkerService::new(
                    nats,
                    open_library,
                    gemini,
                    work_repo.clone(),
                    concept_repo,
                    insight_repo,
                );
                worker.start().await;
                info!("Discovery worker started");
            }
            Err(_) => {
                tracing::warn!("KV_GEMINI_API_KEY not set — discovery worker disabled");
            }
        }
    }

    let user_repo = Arc::new(SurrealUserRepo::new(db.clone()));
    let login_attempt_repo = Arc::new(SurrealLoginAttemptRepo::new(db.clone()));
    let token_repo = Arc::new(SurrealTokenRepo::new(db));
    let state = AppState {
        user_repo,
        login_attempt_repo,
        token_repo,
        work_repo,
        message_publisher,
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
