use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2, Params,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    Form, Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    domain::user::{validate_email, validate_password},
    web::state::AppState,
};

#[derive(Serialize)]
pub struct SetupStatusResponse {
    pub first_run: bool,
}

#[derive(Serialize)]
pub struct SetupCreatedResponse {
    pub user_id: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub status: u16,
}

#[derive(Deserialize)]
pub struct SetupFormInput {
    pub email: String,
    pub password: String,
}

pub async fn get_setup_json(State(state): State<AppState>) -> Response {
    match state.user_repo.count().await {
        Ok(n) => Json(SetupStatusResponse { first_run: n == 0 }).into_response(),
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

pub async fn get_setup(State(state): State<AppState>) -> Response {
    let count = match state.user_repo.count().await {
        Ok(n) => n,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "internal_error".into(),
                    message: e.to_string(),
                    status: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                }),
            )
                .into_response()
        }
    };

    if count > 0 {
        return Redirect::to("/login").into_response();
    }

    Html(setup_html("", "")).into_response()
}

pub async fn post_setup_json(
    State(state): State<AppState>,
    Json(input): Json<SetupFormInput>,
) -> Response {
    handle_setup(state, input.email, input.password, true).await
}

pub async fn post_setup_form(
    State(state): State<AppState>,
    Form(input): Form<SetupFormInput>,
) -> Response {
    handle_setup(state, input.email, input.password, false).await
}

async fn handle_setup(
    state: AppState,
    email: String,
    password: String,
    is_json: bool,
) -> Response {
    // Check if already set up
    let count = match state.user_repo.count().await {
        Ok(n) => n,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                &e.to_string(),
                is_json,
            )
        }
    };

    if count > 0 {
        return error_response(
            StatusCode::CONFLICT,
            "already_configured",
            "An admin account already exists",
            is_json,
        );
    }

    // Validate inputs
    if let Err(e) = validate_email(&email) {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_email",
            &e.to_string(),
            is_json,
        );
    }

    if let Err(e) = validate_password(&password) {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_password",
            &e.to_string(),
            is_json,
        );
    }

    // Hash password with Argon2id (OWASP 2026 params: m=65536, t=3, p=1)
    let password_hash = match hash_password(&password) {
        Ok(h) => h,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                &e,
                is_json,
            )
        }
    };

    // Create user
    match state.user_repo.create(email, password_hash).await {
        Ok(user) => {
            if is_json {
                (
                    StatusCode::CREATED,
                    Json(SetupCreatedResponse { user_id: user.id }),
                )
                    .into_response()
            } else {
                Redirect::to("/login").into_response()
            }
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            &e.to_string(),
            is_json,
        ),
    }
}

fn hash_password(password: &str) -> Result<String, String> {
    let params = Params::new(65536, 3, 1, None)
        .map_err(|e| e.to_string())?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let salt = SaltString::generate(&mut OsRng);
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

fn error_response(status: StatusCode, error: &str, message: &str, is_json: bool) -> Response {
    if is_json {
        (
            status,
            Json(ErrorResponse {
                error: error.into(),
                message: message.into(),
                status: status.as_u16(),
            }),
        )
            .into_response()
    } else {
        Html(setup_html(message, "")).into_response()
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

fn setup_html(error: &str, email: &str) -> String {
    let error_escaped = html_escape(error);
    let email_escaped = html_escape(email);
    let error_html = if error.is_empty() {
        String::new()
    } else {
        format!(r#"<p class="error" role="alert" aria-live="polite">{error_escaped}</p>"#)
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Knowledge Vault — Setup</title>
  <style>
    *, *::before, *::after {{ box-sizing: border-box; }}
    body {{
      font-family: -apple-system, BlinkMacSystemFont, 'BMW Type Next', Arial, sans-serif;
      background: #262626;
      color: #f0f0f0;
      display: flex;
      align-items: center;
      justify-content: center;
      min-height: 100vh;
      margin: 0;
    }}
    .card {{
      background: #1a1a1a;
      border: 1px solid #3c3c3c;
      border-radius: 4px;
      padding: 2.5rem;
      width: 100%;
      max-width: 420px;
    }}
    h1 {{
      font-size: 1.5rem;
      font-weight: 700;
      margin: 0 0 0.5rem;
      color: #f0f0f0;
    }}
    p.subtitle {{
      color: #8c8c8c;
      margin: 0 0 2rem;
      font-size: 0.9rem;
    }}
    label {{
      display: block;
      font-size: 0.85rem;
      font-weight: 600;
      margin-bottom: 0.35rem;
      color: #c8c8c8;
    }}
    input {{
      display: block;
      width: 100%;
      padding: 0.6rem 0.75rem;
      border: 1px solid #3c3c3c;
      border-radius: 2px;
      background: #262626;
      color: #f0f0f0;
      font-size: 0.95rem;
      margin-bottom: 1.25rem;
    }}
    input:focus {{
      outline: 3px solid #0653b6;
      outline-offset: 2px;
    }}
    button {{
      width: 100%;
      padding: 0.7rem;
      background: #1c69d4;
      color: #fff;
      border: none;
      border-radius: 2px;
      font-size: 1rem;
      font-weight: 600;
      cursor: pointer;
    }}
    button:hover {{ background: #0653b6; }}
    button:focus {{
      outline: 3px solid #0653b6;
      outline-offset: 2px;
    }}
    .error {{
      background: #3d1515;
      border: 1px solid #7a2020;
      border-radius: 2px;
      padding: 0.6rem 0.75rem;
      color: #ff8080;
      font-size: 0.9rem;
      margin-bottom: 1rem;
    }}
  </style>
</head>
<body>
  <main class="card">
    <h1>Knowledge Vault</h1>
    <p class="subtitle">Create your admin account to get started.</p>
    {error_html}
    <form method="post" action="/setup">
      <label for="email">Email address</label>
      <input
        id="email"
        type="email"
        name="email"
        value="{email_escaped}"
        required
        autocomplete="email"
        aria-describedby="email-hint"
      />
      <label for="password">Password</label>
      <input
        id="password"
        type="password"
        name="password"
        required
        minlength="12"
        autocomplete="new-password"
        aria-describedby="password-hint"
      />
      <p id="password-hint" style="font-size:0.8rem;color:#8c8c8c;margin-top:-1rem;margin-bottom:1.25rem;">
        Minimum 12 characters.
      </p>
      <button type="submit">Create account</button>
    </form>
  </main>
</body>
</html>"#
    )
}
