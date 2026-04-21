use axum::{
    routing::{delete, get, post},
    Router,
};
use tower::ServiceBuilder;

use super::{
    handlers::{
        add_book::get_add_book,
        auth::post_login,
        health::get_health,
        library::get_library,
        setup::{get_setup, get_setup_json, post_setup_form, post_setup_json},
        tokens::{delete_token, get_tokens, post_token},
        websocket::ws_handler,
        work_detail::{get_work_detail_page, get_work_insight},
        works::{get_work_by_id, get_works, post_works, post_works_retry},
    },
    middleware::security_headers::security_headers_layer,
    state::AppState,
};

pub fn build_router(state: AppState) -> Router {
    let [h1, h2, h3, h4] = security_headers_layer();

    Router::new()
        .route("/health", get(get_health))
        .route("/setup", get(get_setup).post(post_setup_form))
        .route("/api/setup", get(get_setup_json).post(post_setup_json))
        .route("/auth/login", post(post_login))
        .route("/api/tokens", post(post_token).get(get_tokens))
        .route("/api/tokens/{id}", delete(delete_token))
        .route("/api/works", post(post_works).get(get_works))
        .route("/api/works/{id}", get(get_work_by_id))
        .route("/api/works/{id}/retry", post(post_works_retry))
        .route("/api/works/{id}/insight", get(get_work_insight))
        .route("/works/{id}", get(get_work_detail_page))
        .route("/ws", get(ws_handler))
        .route("/add", get(get_add_book))
        .route("/", get(get_library))
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(h1)
                .layer(h2)
                .layer(h3)
                .layer(h4),
        )
}
