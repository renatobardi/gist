use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use email_address::EmailAddress;
use rand::{rngs::OsRng, Rng};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserPreferences {
    pub theme: Option<String>,
    pub default_sort: Option<String>,
    pub default_sort_order: Option<String>,
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub display_name: Option<String>,
    pub preferences: Option<UserPreferences>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: String,
    pub exp: u64,
}

#[derive(Debug, PartialEq)]
pub enum ValidationError {
    PasswordTooShort,
    InvalidEmail(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::PasswordTooShort => {
                write!(f, "Password must be at least 12 characters")
            }
            ValidationError::InvalidEmail(msg) => write!(f, "Invalid email: {msg}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalAccessToken {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

/// Generates a new PAT: `ens_` prefix followed by 32 random bytes, base64url-encoded.
pub fn generate_pat() -> String {
    let bytes: [u8; 32] = rand::thread_rng().gen();
    format!("ens_{}", URL_SAFE_NO_PAD.encode(bytes))
}

/// Hashes a PAT using Argon2id with a random salt for at-rest protection.
pub fn hash_pat(token: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(token.as_bytes(), &salt)
        .expect("argon2 hash must not fail for valid UTF-8 input")
        .to_string()
}

/// Verifies a PAT against a stored Argon2id hash.
pub fn verify_pat(token: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(token.as_bytes(), &parsed)
        .is_ok()
}

pub fn validate_password(password: &str) -> Result<(), ValidationError> {
    if password.len() < 12 {
        return Err(ValidationError::PasswordTooShort);
    }
    Ok(())
}

pub fn validate_email(email: &str) -> Result<(), ValidationError> {
    EmailAddress::parse_with_options(email, email_address::Options::default())
        .map(|_| ())
        .map_err(|e| ValidationError::InvalidEmail(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // RED: password too short
    #[test]
    fn password_under_12_chars_is_invalid() {
        assert_eq!(
            validate_password("short").unwrap_err(),
            ValidationError::PasswordTooShort
        );
    }

    #[test]
    fn password_exactly_11_chars_is_invalid() {
        assert_eq!(
            validate_password("12345678901").unwrap_err(),
            ValidationError::PasswordTooShort
        );
    }

    #[test]
    fn password_exactly_12_chars_is_valid() {
        assert!(validate_password("123456789012").is_ok());
    }

    #[test]
    fn password_over_12_chars_is_valid() {
        assert!(validate_password("this_is_a_valid_password").is_ok());
    }

    // RED: email validation
    #[test]
    fn invalid_email_format_is_rejected() {
        assert!(validate_email("not-an-email").is_err());
    }

    #[test]
    fn email_missing_domain_is_rejected() {
        assert!(validate_email("user@").is_err());
    }

    #[test]
    fn valid_email_is_accepted() {
        assert!(validate_email("user@example.com").is_ok());
    }

    #[test]
    fn valid_email_with_subdomain_is_accepted() {
        assert!(validate_email("admin@mail.example.org").is_ok());
    }

    #[test]
    fn generate_pat_has_ens_prefix() {
        let token = generate_pat();
        assert!(token.starts_with("ens_"), "PAT must start with ens_");
    }

    #[test]
    fn generate_pat_produces_unique_tokens() {
        let a = generate_pat();
        let b = generate_pat();
        assert_ne!(a, b);
    }

    #[test]
    fn hash_pat_uses_different_salt_each_time() {
        let token = "ens_testtoken";
        assert_ne!(
            hash_pat(token),
            hash_pat(token),
            "Argon2id hashes must use unique salts"
        );
    }

    #[test]
    fn hash_pat_differs_for_different_tokens() {
        assert_ne!(hash_pat("ens_aaa"), hash_pat("ens_bbb"));
    }

    #[test]
    fn verify_pat_succeeds_for_correct_token() {
        let token = "ens_testtoken";
        let hash = hash_pat(token);
        assert!(verify_pat(token, &hash));
    }

    #[test]
    fn verify_pat_fails_for_wrong_token() {
        let hash = hash_pat("ens_correct");
        assert!(!verify_pat("ens_wrong", &hash));
    }
}
