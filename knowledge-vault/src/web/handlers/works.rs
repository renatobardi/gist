use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;

use crate::{
    domain::work::{validate_isbn, WorkError},
    web::{middleware::auth::AuthenticatedUser, state::AppState},
};

#[derive(Deserialize)]
pub struct SubmitWorkInput {
    pub identifier: String,
    pub identifier_type: String,
}

#[derive(Serialize)]
pub struct WorkCreatedResponse {
    pub work_id: String,
    pub status: String,
}

pub async fn post_works(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Json(input): Json<SubmitWorkInput>,
) -> Response {
    if input.identifier_type != "isbn" {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": "invalid_identifier_type",
                "message": "identifier_type must be \"isbn\""
            })),
        )
            .into_response();
    }

    let isbn = match validate_isbn(&input.identifier) {
        Ok(normalised) => normalised,
        Err(WorkError::InvalidIsbn(msg)) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "error": "invalid_isbn", "message": msg })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal_error", "message": e.to_string() })),
            )
                .into_response();
        }
    };

    match state.work_repo.find_by_isbn(&isbn).await {
        Ok(Some(existing)) => {
            info!(isbn = %isbn, existing_work_id = %existing.id, "duplicate ISBN, returning 409");
            return (
                StatusCode::CONFLICT,
                Json(json!({ "work_id": existing.id, "error": "duplicate" })),
            )
                .into_response();
        }
        Ok(None) => {}
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal_error", "message": e.to_string() })),
            )
                .into_response();
        }
    }

    let work = match state.work_repo.create_work(&isbn).await {
        Ok(w) => w,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal_error", "message": e.to_string() })),
            )
                .into_response();
        }
    };

    let publisher = match &state.message_publisher {
        Some(p) => p,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "messaging_unavailable",
                    "message": "NATS publisher is not initialised"
                })),
            )
                .into_response();
        }
    };

    let event = serde_json::to_vec(&json!({
        "work_id": work.id,
        "identifier": isbn,
        "identifier_type": "isbn"
    }))
    .expect("serialisation of a known-good value must not fail");

    if let Err(e) = publisher.publish("discovery.requested", event).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "messaging_error", "message": e })),
        )
            .into_response();
    }

    info!(work_id = %work.id, isbn = %isbn, "work created, discovery.requested published");

    (
        StatusCode::ACCEPTED,
        Json(WorkCreatedResponse {
            work_id: work.id,
            status: work.status,
        }),
    )
        .into_response()
}
