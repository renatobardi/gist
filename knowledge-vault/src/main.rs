use knowledge_vault::{adapters, app, web};

use std::sync::Arc;

use surrealdb::{engine::local::Db, Surreal};
use tracing::info;
use tracing_subscriber::EnvFilter;

use async_nats::jetstream::{self, consumer::pull, stream};

use adapters::{
    gemini::GeminiClient,
    nats::{consumer::NatsConsumer, publisher::NatsPublisher},
    openlib::OpenLibraryClient,
    surreal::{
        concept_repo::SurrealConceptRepo, graph_write_repo::SurrealGraphWriteRepo,
        insight_repo::SurrealInsightRepo, login_attempt_repo::SurrealLoginAttemptRepo,
        schema::run_migrations, token_repo::SurrealTokenRepo, user_repo::SurrealUserRepo,
        work_repo::SurrealWorkRepo,
    },
};
use app::worker::WorkerService;
use web::{router::build_router, state::AppState, ws_broadcaster::WsBroadcaster};

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
    let work_repo: Arc<dyn knowledge_vault::ports::repository::WorkRepo> =
        Arc::new(SurrealWorkRepo::new(db.clone()));
    let insight_repo: Arc<dyn knowledge_vault::ports::repository::InsightRepo> =
        Arc::new(SurrealInsightRepo::new(db.clone()));
    let concept_repo: Arc<dyn knowledge_vault::ports::repository::ConceptRepo> =
        Arc::new(SurrealConceptRepo::new(db.clone()));
    let graph_write_repo: Arc<dyn knowledge_vault::ports::repository::GraphWriteRepo> =
        Arc::new(SurrealGraphWriteRepo::new(db.clone()));

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

    // Start NATS worker if Gemini API key is set
    let gemini_api_key = std::env::var("KV_GEMINI_API_KEY").ok();
    match gemini_api_key {
        None => {
            tracing::warn!("KV_GEMINI_API_KEY not set — NATS worker will not start");
        }
        Some(api_key) => match async_nats::connect(&nats_url).await {
            Err(e) => {
                tracing::warn!("NATS unavailable for worker — worker will not start: {e}");
            }
            Ok(nats_client) => {
                let js = jetstream::new(nats_client);
                let consumer_result: anyhow::Result<_> = async {
                    let nats_stream = js
                        .get_or_create_stream(stream::Config {
                            name: "DISCOVERY".to_string(),
                            subjects: vec!["discovery.requested".to_string()],
                            ..Default::default()
                        })
                        .await
                        .map_err(anyhow::Error::from)?;
                    nats_stream
                        .get_or_create_consumer(
                            "worker",
                            pull::Config {
                                durable_name: Some("worker".to_string()),
                                ..Default::default()
                            },
                        )
                        .await
                        .map_err(anyhow::Error::from)
                }
                .await;
                match consumer_result {
                    Err(e) => {
                        tracing::warn!(
                            "Failed to create NATS consumer — worker will not start: {e}"
                        );
                    }
                    Ok(js_consumer) => {
                        let consumer = NatsConsumer::new(js_consumer);
                        let model = std::env::var("KV_GEMINI_MODEL")
                            .unwrap_or_else(|_| "gemini-2.0-flash".to_string());
                        let openlib = Arc::new(
                            OpenLibraryClient::build()
                                .expect("failed to build OpenLibraryClient for worker"),
                        )
                            as Arc<dyn knowledge_vault::ports::external::OpenLibraryPort>;
                        let gemini = Arc::new(GeminiClient::new(api_key, model))
                            as Arc<dyn knowledge_vault::ports::external::GeminiPort>;

                        let worker = Arc::new(WorkerService::new(
                            work_repo.clone(),
                            graph_write_repo.clone(),
                            openlib,
                            gemini,
                        ));

                        info!("Starting NATS worker");
                        worker.spawn(consumer);
                    }
                }
            }
        },
    }

    let ws_broadcaster = WsBroadcaster::new();
    let state = AppState {
        user_repo,
        login_attempt_repo,
        token_repo,
        work_repo,
        insight_repo,
        concept_repo,
        graph_write_repo,
        message_publisher,
        open_library_client,
        ws_broadcaster,
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
