use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};

use crate::{domain::user::AuthClaims, web::state::AppState};

const LOCKOUT_MAX_FAILURES: u64 = 3;
const LOCKOUT_WINDOW_SECONDS: u64 = 300;

#[derive(Deserialize)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub status: u16,
}

#[derive(Serialize)]
pub struct RateLimitResponse {
    pub error: String,
    pub retry_after_seconds: u64,
}

pub async fn post_login(State(state): State<AppState>, Json(input): Json<LoginInput>) -> Response {
    let email = input.email.trim().to_lowercase();

    // Check rate limit: 3 failures in 5 minutes.
    // Note: count-then-record is non-atomic; under high concurrency a burst of concurrent
    // requests at exactly N-1 failures could each pass the check. Acceptable for single-server MVP.
    let failure_count = match state
        .login_attempt_repo
        .count_recent_failures(&email, LOCKOUT_WINDOW_SECONDS)
        .await
    {
        Ok(n) => n,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "internal_error".into(),
                    message: e.to_string(),
                    status: 500,
                }),
            )
                .into_response()
        }
    };

    if failure_count >= LOCKOUT_MAX_FAILURES {
        let retry_after = compute_retry_after(&state, &email).await;
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(RateLimitResponse {
                error: "rate_limited".into(),
                retry_after_seconds: retry_after,
            }),
        )
            .into_response();
    }

    // Look up user
    let user = match state.user_repo.find_by_email(&email).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            let _ = state.login_attempt_repo.record(&email, false).await;
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "invalid_credentials".into(),
                    message: "Invalid email or password".into(),
                    status: 401,
                }),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "internal_error".into(),
                    message: e.to_string(),
                    status: 500,
                }),
            )
                .into_response()
        }
    };

    // Verify password
    let parsed_hash = match PasswordHash::new(&user.password_hash) {
        Ok(h) => h,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "internal_error".into(),
                    message: "Password hash corrupted".into(),
                    status: 500,
                }),
            )
                .into_response()
        }
    };

    let valid = Argon2::default()
        .verify_password(input.password.as_bytes(), &parsed_hash)
        .is_ok();

    if !valid {
        let _ = state.login_attempt_repo.record(&email, false).await;
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "invalid_credentials".into(),
                message: "Invalid email or password".into(),
                status: 401,
            }),
        )
            .into_response();
    }

    let _ = state.login_attempt_repo.record(&email, true).await;

    // Issue JWT
    let exp = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as u64;

    let claims = AuthClaims {
        sub: user.id.clone(),
        exp,
    };

    let token = match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    ) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "internal_error".into(),
                    message: e.to_string(),
                    status: 500,
                }),
            )
                .into_response()
        }
    };

    let cookie = format!(
        "session={}; HttpOnly; Secure; SameSite=Strict; Path=/",
        token
    );

    (
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        Json(LoginResponse { token }),
    )
        .into_response()
}

async fn compute_retry_after(state: &AppState, email: &str) -> u64 {
    match state
        .login_attempt_repo
        .oldest_recent_failure_ts(email, LOCKOUT_WINDOW_SECONDS)
        .await
    {
        Ok(Some(oldest_ts)) => {
            let now = chrono::Utc::now().timestamp();
            let lockout_end = oldest_ts + LOCKOUT_WINDOW_SECONDS as i64;
            let remaining = lockout_end - now;
            if remaining > 0 { remaining as u64 } else { 0 }
        }
        _ => LOCKOUT_WINDOW_SECONDS,
    }
}
