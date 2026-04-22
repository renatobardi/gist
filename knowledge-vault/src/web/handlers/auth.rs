use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
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

pub async fn get_login() -> impl IntoResponse {
    Html(LOGIN_HTML)
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
            if remaining > 0 {
                remaining as u64
            } else {
                0
            }
        }
        _ => LOCKOUT_WINDOW_SECONDS,
    }
}

const LOGIN_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Knowledge Vault — Login</title>
  <style>
    *, *::before, *::after { box-sizing: border-box; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'BMW Type Next', Inter, Arial, sans-serif;
      background: #262626;
      color: #f0f0f0;
      display: flex;
      align-items: center;
      justify-content: center;
      min-height: 100vh;
      margin: 0;
    }
    .card {
      background: #1a1a1a;
      border: 1px solid #3c3c3c;
      border-radius: 4px;
      padding: 2.5rem;
      width: 100%;
      max-width: 420px;
    }
    h1 {
      font-size: 1.5rem;
      font-weight: 700;
      margin: 0 0 0.5rem;
    }
    p.subtitle {
      color: #8c8c8c;
      margin: 0 0 2rem;
      font-size: 0.9rem;
    }
    label {
      display: block;
      font-size: 0.85rem;
      font-weight: 600;
      margin-bottom: 0.35rem;
      color: #c8c8c8;
    }
    input {
      display: block;
      width: 100%;
      padding: 0.6rem 0.75rem;
      border: 1px solid #3c3c3c;
      border-radius: 2px;
      background: #262626;
      color: #f0f0f0;
      font-size: 0.95rem;
      margin-bottom: 1.25rem;
      font-family: inherit;
    }
    input:focus { outline: 3px solid #0653b6; outline-offset: 2px; }
    button {
      width: 100%;
      padding: 0.7rem;
      background: #1c69d4;
      color: #fff;
      border: none;
      border-radius: 2px;
      font-size: 1rem;
      font-weight: 600;
      cursor: pointer;
      font-family: inherit;
    }
    button:hover { background: #0653b6; }
    button:focus { outline: 3px solid #0653b6; outline-offset: 2px; }
    button:disabled { background: #3c3c3c; cursor: not-allowed; }
    .error {
      background: #3d1515;
      border: 1px solid #7a2020;
      border-radius: 2px;
      padding: 0.6rem 0.75rem;
      color: #ff8080;
      font-size: 0.9rem;
      margin-bottom: 1rem;
      display: none;
    }
    .error.visible { display: block; }
  </style>
</head>
<body>
  <main class="card">
    <h1>Knowledge Vault</h1>
    <p class="subtitle">Sign in to your account.</p>
    <div id="error" class="error" role="alert" aria-live="polite"></div>
    <form id="form">
      <label for="email">Email address</label>
      <input id="email" type="email" name="email" required autocomplete="email" />
      <label for="password">Password</label>
      <input id="password" type="password" name="password" required autocomplete="current-password" />
      <button type="submit" id="btn">Sign in</button>
    </form>
  </main>
  <script>
    document.getElementById('form').addEventListener('submit', async (e) => {
      e.preventDefault();
      const btn = document.getElementById('btn');
      const errorEl = document.getElementById('error');
      btn.disabled = true;
      btn.textContent = 'Signing in…';
      errorEl.classList.remove('visible');

      try {
        const res = await fetch('/auth/login', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            email: document.getElementById('email').value,
            password: document.getElementById('password').value,
          }),
        });

        if (res.ok) {
          window.location.href = '/';
          return;
        }

        const data = await res.json().catch(() => ({}));
        if (res.status === 429) {
          const secs = data.retry_after_seconds ?? 300;
          const mins = Math.ceil(secs / 60);
          errorEl.textContent = `Too many failed attempts. Try again in ${mins} minute${mins !== 1 ? 's' : ''}.`;
        } else {
          errorEl.textContent = data.message ?? 'Invalid email or password.';
        }
        errorEl.classList.add('visible');
      } catch {
        errorEl.textContent = 'Network error. Please try again.';
        errorEl.classList.add('visible');
      } finally {
        btn.disabled = false;
        btn.textContent = 'Sign in';
      }
    });
  </script>
</body>
</html>"#;
