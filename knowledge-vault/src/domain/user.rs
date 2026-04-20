use email_address::EmailAddress;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub role: String,
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
}
