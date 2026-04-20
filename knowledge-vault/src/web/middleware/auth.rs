use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Json, Response},
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde_json::json;

use crate::{
    domain::user::{hash_pat, AuthClaims},
    web::state::AppState,
};

pub struct AuthenticatedUser {
    pub user_id: String,
}

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = extract_bearer(parts).or_else(|| extract_session_cookie(parts));

        let token = match token {
            Some(t) => t,
            None => return Err(unauthorized("missing_token", "Authentication required")),
        };

        if token.starts_with("ens_") {
            validate_pat(token, state).await
        } else {
            validate_jwt(token, state)
        }
    }
}

fn extract_bearer(parts: &Parts) -> Option<String> {
    let header = parts.headers.get(axum::http::header::AUTHORIZATION)?;
    let value = header.to_str().ok()?;
    value.strip_prefix("Bearer ").map(|s| s.to_string())
}

fn extract_session_cookie(parts: &Parts) -> Option<String> {
    let header = parts.headers.get(axum::http::header::COOKIE)?;
    let value = header.to_str().ok()?;
    value
        .split(';')
        .find_map(|part| {
            let part = part.trim();
            part.strip_prefix("session=").map(|v| v.to_string())
        })
}

fn validate_jwt(token: String, state: &AppState) -> Result<AuthenticatedUser, Response> {
    let decoded = decode::<AuthClaims>(
        &token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| unauthorized("invalid_token", "Invalid or expired token"))?;

    Ok(AuthenticatedUser {
        user_id: decoded.claims.sub,
    })
}

async fn validate_pat(token: String, state: &AppState) -> Result<AuthenticatedUser, Response> {
    let hash = hash_pat(&token);

    let pat = state
        .token_repo
        .find_by_hash(&hash)
        .await
        .map_err(|_| unauthorized("invalid_token", "Invalid token"))?
        .ok_or_else(|| unauthorized("invalid_token", "Invalid token"))?;

    Ok(AuthenticatedUser {
        user_id: pat.user_id,
    })
}

fn unauthorized(error: &str, message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": error, "message": message, "status": 401 })),
    )
        .into_response()
}
