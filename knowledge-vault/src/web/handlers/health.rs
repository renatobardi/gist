use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use crate::{config::VERSION, web::state::AppState};

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db: Option<&'static str>,
}

pub async fn get_health(State(state): State<AppState>) -> Response {
    let db_ok = state.db.query("RETURN 1").await.is_ok();

    if db_ok {
        (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ok",
                version: VERSION,
                db: Some("connected"),
            }),
        )
            .into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                status: "degraded",
                version: VERSION,
                db: Some("disconnected"),
            }),
        )
            .into_response()
    }
}
