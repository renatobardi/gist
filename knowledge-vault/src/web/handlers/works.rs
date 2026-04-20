use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::Deserialize as QueryDeserialize;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;

use crate::{
    domain::work::{validate_isbn, WorkError},
    ports::repository::RepoError,
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
    match input.identifier_type.as_str() {
        "isbn" => handle_isbn(state, input.identifier).await,
        "title" => handle_title(state, input.identifier).await,
        _ => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": "invalid_identifier_type",
                "message": "identifier_type must be \"isbn\" or \"title\""
            })),
        )
            .into_response(),
    }
}

async fn handle_isbn(state: AppState, identifier: String) -> Response {
    let isbn = match validate_isbn(&identifier) {
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

    let publisher = match &state.message_publisher {
        Some(p) => p.clone(),
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

    let work = match state.work_repo.create_work(&isbn).await {
        Ok(w) => w,
        Err(e) => {
            if let Ok(Some(existing)) = state.work_repo.find_by_isbn(&isbn).await {
                info!(isbn = %isbn, existing_work_id = %existing.id, "duplicate ISBN detected on race, returning 409");
                return (
                    StatusCode::CONFLICT,
                    Json(json!({ "work_id": existing.id, "error": "duplicate" })),
                )
                    .into_response();
            }
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal_error", "message": e.to_string() })),
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

async fn handle_title(state: AppState, title: String) -> Response {
    if title.trim().is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": "invalid_title", "message": "title must not be empty" })),
        )
            .into_response();
    }

    let ol_client = match &state.open_library_client {
        Some(c) => c.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "service_unavailable",
                    "message": "Open Library client is not initialised"
                })),
            )
                .into_response();
        }
    };

    let book = match ol_client.search_by_title(&title).await {
        Ok(Some(b)) => b,
        Ok(None) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "error": "title_not_found", "message": "no results found for the given title" })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal_error", "message": e })),
            )
                .into_response();
        }
    };

    match state
        .work_repo
        .find_by_open_library_id(&book.open_library_id)
        .await
    {
        Ok(Some(existing)) => {
            info!(ol_id = %book.open_library_id, existing_work_id = %existing.id, "duplicate open_library_id, returning 409");
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

    let publisher = match &state.message_publisher {
        Some(p) => p.clone(),
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

    let work = match state
        .work_repo
        .create_work_by_title(&book.title, &book.author, &book.open_library_id)
        .await
    {
        Ok(w) => w,
        Err(e) => {
            if let Ok(Some(existing)) = state
                .work_repo
                .find_by_open_library_id(&book.open_library_id)
                .await
            {
                info!(ol_id = %book.open_library_id, existing_work_id = %existing.id, "duplicate open_library_id detected on race, returning 409");
                return (
                    StatusCode::CONFLICT,
                    Json(json!({ "work_id": existing.id, "error": "duplicate" })),
                )
                    .into_response();
            }
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal_error", "message": e.to_string() })),
            )
                .into_response();
        }
    };

    let event = serde_json::to_vec(&json!({
        "work_id": work.id,
        "identifier": title,
        "identifier_type": "title"
    }))
    .expect("serialisation of a known-good value must not fail");

    if let Err(e) = publisher.publish("discovery.requested", event).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "messaging_error", "message": e })),
        )
            .into_response();
    }

    info!(work_id = %work.id, title = %title, "work created by title, discovery.requested published");

    (
        StatusCode::ACCEPTED,
        Json(WorkCreatedResponse {
            work_id: work.id,
            status: work.status,
        }),
    )
        .into_response()
}

pub async fn post_works_retry(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Path(id): Path<String>,
) -> Response {
    let work = match state.work_repo.find_by_id(&id).await {
        Ok(Some(w)) => w,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, Json(json!({ "error": "not_found" }))).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal_error", "message": e.to_string() })),
            )
                .into_response();
        }
    };

    if work.status != "failed" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "not_failed" })),
        )
            .into_response();
    }

    let publisher = match &state.message_publisher {
        Some(p) => p.clone(),
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

    let updated = match state.work_repo.reset_to_pending(&id).await {
        Ok(w) => w,
        Err(RepoError::NotFound) => {
            return (
                StatusCode::CONFLICT,
                Json(json!({ "error": "status_changed" })),
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

    let event = serde_json::to_vec(&json!({
        "work_id": updated.id,
        "identifier": updated.isbn,
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

    info!(work_id = %updated.id, "manual retry triggered, discovery.requested published");

    (
        StatusCode::ACCEPTED,
        Json(WorkCreatedResponse {
            work_id: updated.id,
            status: updated.status,
        }),
    )
        .into_response()
}

#[derive(QueryDeserialize)]
pub struct ListWorksParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub async fn get_works(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Query(params): Query<ListWorksParams>,
) -> Response {
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    match state.work_repo.list_works(limit, offset).await {
        Ok(works) => Json(works).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal_error", "message": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn get_work_by_id(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Path(id): Path<String>,
) -> Response {
    match state.work_repo.get_work_by_id(&id).await {
        Ok(Some(work)) => Json(work).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "not_found", "message": "Work not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal_error", "message": e.to_string() })),
        )
            .into_response(),
    }
}
