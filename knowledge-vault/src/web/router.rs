use axum::{
    routing::{delete, get, post},
    Router,
};
use tower::ServiceBuilder;

use super::{
    handlers::{
        auth::post_login,
        setup::{get_setup, get_setup_json, post_setup_form, post_setup_json},
        tokens::{delete_token, get_tokens, post_token},
        works::post_works,
    },
    middleware::security_headers::security_headers_layer,
    state::AppState,
};

pub fn build_router(state: AppState) -> Router {
    let [h1, h2, h3, h4] = security_headers_layer();

    Router::new()
        .route("/setup", get(get_setup).post(post_setup_form))
        .route("/api/setup", get(get_setup_json).post(post_setup_json))
        .route("/auth/login", post(post_login))
        .route("/api/tokens", post(post_token).get(get_tokens))
        .route("/api/tokens/{id}", delete(delete_token))
        .route("/api/works", post(post_works))
        .route("/", get(root_redirect))
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(h1)
                .layer(h2)
                .layer(h3)
                .layer(h4),
        )
}

async fn root_redirect(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> axum::response::Response {
    use axum::response::{IntoResponse, Redirect};
    match state.user_repo.count().await {
        Ok(0) => Redirect::to("/setup").into_response(),
        _ => Redirect::to("/login").into_response(),
    }
}
