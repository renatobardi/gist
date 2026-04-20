use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};

use crate::{
    domain::user::{generate_pat, hash_pat},
    ports::repository::RepoError,
    web::{middleware::auth::AuthenticatedUser, state::AppState},
};

#[derive(Deserialize)]
pub struct CreateTokenInput {
    pub name: String,
}

#[derive(Serialize)]
pub struct CreateTokenResponse {
    pub token_id: String,
    pub token: String,
    pub name: String,
}

#[derive(Serialize)]
pub struct TokenListItem {
    pub token_id: String,
    pub name: String,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub status: u16,
}

pub async fn post_token(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(input): Json<CreateTokenInput>,
) -> Response {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse {
                error: "invalid_input".into(),
                message: "Token name must not be empty".into(),
                status: 422,
            }),
        )
            .into_response();
    }

    let token = generate_pat();
    let hash = hash_pat(&token);

    match state.token_repo.create(&auth.user_id, name.clone(), hash).await {
        Ok(token_id) => (
            StatusCode::CREATED,
            Json(CreateTokenResponse {
                token_id,
                token,
                name,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "internal_error".into(),
                message: e.to_string(),
                status: 500,
            }),
        )
            .into_response(),
    }
}

pub async fn get_tokens(State(state): State<AppState>, auth: AuthenticatedUser) -> Response {
    match state.token_repo.list(&auth.user_id).await {
        Ok(tokens) => {
            let items: Vec<TokenListItem> = tokens
                .into_iter()
                .map(|t| TokenListItem {
                    token_id: t.id,
                    name: t.name,
                    created_at: t.created_at,
                    revoked_at: t.revoked_at,
                })
                .collect();
            Json(items).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "internal_error".into(),
                message: e.to_string(),
                status: 500,
            }),
        )
            .into_response(),
    }
}

pub async fn delete_token(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(token_id): Path<String>,
) -> Response {
    match state.token_repo.revoke(&token_id, &auth.user_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(RepoError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "not_found".into(),
                message: "Token not found".into(),
                status: 404,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "internal_error".into(),
                message: e.to_string(),
                status: 500,
            }),
        )
            .into_response(),
    }
}
