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
    domain::insight::{ExtractedConcept, GeminiResponse},
    ports::{messaging::MessagePublisher, repository::GraphWriteRepo, repository::WorkRepo},
    web::{router::build_router, state::AppState, ws_broadcaster::WsBroadcaster},
};

struct NoopPublisher;

#[async_trait]
impl MessagePublisher for NoopPublisher {
    async fn publish(&self, _subject: &str, _payload: Vec<u8>) -> Result<(), String> {
        Ok(())
    }
}

type TestDb = surrealdb::Surreal<surrealdb::engine::local::Db>;

async fn make_test_server_with_db() -> (
    TestServer,
    TestDb,
    Arc<dyn GraphWriteRepo>,
    Arc<dyn WorkRepo>,
) {
    let db: TestDb = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("kv_test").use_db("kv_test").await.unwrap();
    run_migrations(&db).await.unwrap();

    let work_repo = Arc::new(SurrealWorkRepo::new(db.clone()));
    let graph_write_repo = Arc::new(SurrealGraphWriteRepo::new(db.clone()));

    let state = AppState {
        db: Arc::new(db.clone()),
        user_repo: Arc::new(SurrealUserRepo::new(db.clone())),
        login_attempt_repo: Arc::new(SurrealLoginAttemptRepo::new(db.clone())),
        token_repo: Arc::new(SurrealTokenRepo::new(db.clone())),
        work_repo: work_repo.clone(),
        insight_repo: Arc::new(SurrealInsightRepo::new(db.clone())),
        concept_repo: Arc::new(SurrealConceptRepo::new(db.clone())),
        graph_write_repo: graph_write_repo.clone(),
        graph_read_repo: Arc::new(SurrealGraphReadRepo::new(db.clone())),
        message_publisher: Some(Arc::new(NoopPublisher)),
        open_library_client: None,
        ws_broadcaster: WsBroadcaster::new(),
        jwt_secret: "test-secret".to_string(),
    };

    (
        TestServer::new(build_router(state)).unwrap(),
        db,
        graph_write_repo,
        work_repo,
    )
}

async fn make_test_server() -> TestServer {
    let (server, _, _, _) = make_test_server_with_db().await;
    server
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

fn economics_concept(name: &str, display_name: &str) -> ExtractedConcept {
    ExtractedConcept {
        name: name.to_string(),
        display_name: display_name.to_string(),
        description: "An economics concept.".to_string(),
        domain: "Economics".to_string(),
        relevance_weight: 0.9,
        related_concepts: vec![],
    }
}

fn philosophy_concept(name: &str, display_name: &str) -> ExtractedConcept {
    ExtractedConcept {
        name: name.to_string(),
        display_name: display_name.to_string(),
        description: "A philosophy concept.".to_string(),
        domain: "Philosophy".to_string(),
        relevance_weight: 0.8,
        related_concepts: vec![],
    }
}

async fn seed_graph_data(
    work_repo: &Arc<dyn WorkRepo>,
    graph_write_repo: &Arc<dyn GraphWriteRepo>,
) {
    let work1 = work_repo
        .create_work_by_title("Wealth of Nations", "Adam Smith", "OL1234567M")
        .await
        .unwrap();

    let economics_response = GeminiResponse {
        summary: "A foundational economics text.".to_string(),
        key_points: vec!["Supply and demand".to_string()],
        concepts: vec![
            economics_concept("supply and demand", "Supply and Demand"),
            economics_concept("market equilibrium", "Market Equilibrium"),
        ],
    };

    graph_write_repo
        .write_graph_transaction(&work1.id, &economics_response)
        .await
        .unwrap();

    let work2 = work_repo
        .create_work_by_title("Being and Time", "Martin Heidegger", "OL9876543M")
        .await
        .unwrap();

    let philosophy_response = GeminiResponse {
        summary: "A foundational philosophy text.".to_string(),
        key_points: vec!["Being and Time".to_string()],
        concepts: vec![
            philosophy_concept("dasein", "Dasein"),
            philosophy_concept("phenomenology", "Phenomenology"),
        ],
    };

    graph_write_repo
        .write_graph_transaction(&work2.id, &philosophy_response)
        .await
        .unwrap();
}

// ── GET /api/graph ───────────────────────────────────────────────────────────

#[tokio::test]
async fn get_api_graph_authenticated_empty_returns_200() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;
    let res = server
        .get("/api/graph")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    assert_eq!(res.status_code(), StatusCode::OK);
    let body = res.json::<serde_json::Value>();
    assert!(body["nodes"].is_array(), "expected nodes array");
    assert!(body["edges"].is_array(), "expected edges array");
    assert_eq!(body["nodes"].as_array().unwrap().len(), 0);
    assert_eq!(body["edges"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn get_api_graph_unauthenticated_returns_401() {
    let server = make_test_server().await;
    let res = server.get("/api/graph").await;
    assert_eq!(res.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn get_api_graph_returns_all_nodes_without_filter() {
    let (server, _, graph_write_repo, work_repo) = make_test_server_with_db().await;
    let jwt = setup_and_login(&server).await;

    seed_graph_data(&work_repo, &graph_write_repo).await;

    let res = server
        .get("/api/graph")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    assert_eq!(res.status_code(), StatusCode::OK);
    let body = res.json::<serde_json::Value>();
    let nodes = body["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 4, "expected 4 nodes across 2 domains");
    let edges = body["edges"].as_array().unwrap();
    assert_eq!(
        edges.len(),
        0,
        "expected 0 edges when no related concepts seeded"
    );
}

#[tokio::test]
async fn get_api_graph_domain_filter_returns_only_matching_nodes() {
    let (server, _, graph_write_repo, work_repo) = make_test_server_with_db().await;
    let jwt = setup_and_login(&server).await;

    seed_graph_data(&work_repo, &graph_write_repo).await;

    let res = server
        .get("/api/graph?domain=Economics")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    assert_eq!(res.status_code(), StatusCode::OK);
    let body = res.json::<serde_json::Value>();
    let nodes = body["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2, "expected 2 Economics nodes");
    for node in nodes {
        assert_eq!(
            node["domain"].as_str().unwrap(),
            "Economics",
            "all returned nodes must be Economics domain"
        );
    }
}

#[tokio::test]
async fn get_api_graph_domain_filter_multi_domain_returns_matching_nodes() {
    let (server, _, graph_write_repo, work_repo) = make_test_server_with_db().await;
    let jwt = setup_and_login(&server).await;

    seed_graph_data(&work_repo, &graph_write_repo).await;

    let res = server
        .get("/api/graph?domain=Economics,Philosophy")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    assert_eq!(res.status_code(), StatusCode::OK);
    let body = res.json::<serde_json::Value>();
    let nodes = body["nodes"].as_array().unwrap();
    assert_eq!(
        nodes.len(),
        4,
        "expected all 4 nodes when both domains selected"
    );

    let domains: Vec<&str> = nodes
        .iter()
        .map(|n| n["domain"].as_str().unwrap())
        .collect();
    assert!(
        domains.contains(&"Economics"),
        "Economics domain should be present"
    );
    assert!(
        domains.contains(&"Philosophy"),
        "Philosophy domain should be present"
    );
}

#[tokio::test]
async fn get_api_graph_domain_filter_unknown_domain_returns_empty() {
    let (server, _, graph_write_repo, work_repo) = make_test_server_with_db().await;
    let jwt = setup_and_login(&server).await;

    seed_graph_data(&work_repo, &graph_write_repo).await;

    let res = server
        .get("/api/graph?domain=Astrophysics")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    assert_eq!(res.status_code(), StatusCode::OK);
    let body = res.json::<serde_json::Value>();
    let nodes = body["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 0, "unknown domain should return no nodes");
}

#[tokio::test]
async fn get_api_graph_edges_filtered_when_domain_applied() {
    let (server, _, graph_write_repo, work_repo) = make_test_server_with_db().await;
    let jwt = setup_and_login(&server).await;

    // Seed a work whose concepts have cross-domain related_concepts
    let work = work_repo
        .create_work_by_title("Cross Domain Book", "Author X", "OL1111111M")
        .await
        .unwrap();

    let response = GeminiResponse {
        summary: "A cross-domain book.".to_string(),
        key_points: vec![],
        concepts: vec![
            ExtractedConcept {
                name: "opportunity cost".to_string(),
                display_name: "Opportunity Cost".to_string(),
                description: "Economics concept.".to_string(),
                domain: "Economics".to_string(),
                relevance_weight: 0.9,
                related_concepts: vec![knowledge_vault::domain::insight::RelatedConceptRef {
                    name: "rationality".to_string(),
                    relation_type: "related".to_string(),
                    strength: 0.7,
                }],
            },
            ExtractedConcept {
                name: "rationality".to_string(),
                display_name: "Rationality".to_string(),
                description: "Philosophy concept.".to_string(),
                domain: "Philosophy".to_string(),
                relevance_weight: 0.8,
                related_concepts: vec![],
            },
        ],
    };

    graph_write_repo
        .write_graph_transaction(&work.id, &response)
        .await
        .unwrap();

    // All nodes: 2 nodes, 1 cross-domain edge
    let res = server
        .get("/api/graph")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    let body = res.json::<serde_json::Value>();
    assert_eq!(body["nodes"].as_array().unwrap().len(), 2);
    // Cross-domain edge exists when no filter
    let total_edges = body["edges"].as_array().unwrap().len();
    assert!(
        total_edges >= 1,
        "edge should exist between cross-domain concepts"
    );

    // Filter to Economics only: edge connecting to Philosophy node must be removed
    let res = server
        .get("/api/graph?domain=Economics")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    let body = res.json::<serde_json::Value>();
    let nodes = body["nodes"].as_array().unwrap();
    let edges = body["edges"].as_array().unwrap();
    assert_eq!(
        nodes.len(),
        1,
        "only Economics node when filtering by Economics"
    );
    assert_eq!(
        edges.len(),
        0,
        "cross-domain edge must be excluded when one endpoint is filtered out"
    );
}

// ── GET /graph (HTML page) ───────────────────────────────────────────────────

#[tokio::test]
async fn get_graph_page_authenticated_returns_200() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;
    let res = server
        .get("/graph")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    assert_eq!(res.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn get_graph_page_returns_html_content_type() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;
    let res = server
        .get("/graph")
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
async fn get_graph_page_contains_domain_filter_chip_container() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;
    let res = server
        .get("/graph")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    let body = res.text();
    assert!(
        body.contains("domain-chips"),
        "missing domain-chips container"
    );
    assert!(body.contains("Domain"), "missing Domain filter label");
    assert!(
        body.contains("data-domain"),
        "missing data-domain attributes"
    );
}

#[tokio::test]
async fn get_graph_page_contains_accessibility_attributes() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;
    let res = server
        .get("/graph")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    let body = res.text();
    assert!(body.contains("aria-label"), "missing aria-label");
    assert!(
        body.contains("aria-live"),
        "missing aria-live on loading state"
    );
    assert!(
        body.contains("role=\"checkbox\""),
        "chips must use role=checkbox"
    );
    assert!(
        body.contains("aria-checked"),
        "chips must have aria-checked attribute"
    );
}

#[tokio::test]
async fn get_graph_page_contains_zoom_controls() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;
    let res = server
        .get("/graph")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    let body = res.text();
    assert!(body.contains("zoom-in"), "missing zoom-in button");
    assert!(body.contains("zoom-out"), "missing zoom-out button");
    assert!(body.contains("zoom-reset"), "missing zoom reset/fit button");
}

#[tokio::test]
async fn get_graph_page_unauthenticated_redirects_to_login() {
    let server = make_test_server().await;
    server
        .post("/api/setup")
        .json(&json!({"email": "admin@example.com", "password": "validpassword1"}))
        .await
        .assert_status(StatusCode::CREATED);

    let res = server.get("/graph").await;
    assert_eq!(res.status_code(), StatusCode::SEE_OTHER);
    let location = res
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/login", "expected redirect to /login");
}

#[tokio::test]
async fn get_graph_page_no_users_redirects_to_setup() {
    let server = make_test_server().await;
    let res = server.get("/graph").await;
    assert_eq!(res.status_code(), StatusCode::SEE_OTHER);
    let location = res
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/setup", "expected redirect to /setup");
}

#[tokio::test]
async fn get_graph_page_contains_nav_links() {
    let server = make_test_server().await;
    let jwt = setup_and_login(&server).await;
    let res = server
        .get("/graph")
        .add_header("Authorization", format!("Bearer {jwt}"))
        .await;
    let body = res.text();
    assert!(body.contains("href=\"/\""), "missing Library nav link");
    assert!(body.contains("href=\"/graph\""), "missing Graph nav link");
}
