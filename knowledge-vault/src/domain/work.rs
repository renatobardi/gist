use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Work {
    pub id: String,
    pub title: String,
    pub author: String,
    pub isbn: Option<String>,
    pub open_library_id: Option<String>,
    pub status: String,
    pub error_msg: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub progress_pct: i32,
    pub last_action: String,
    pub reading_status: Option<String>,
    pub cover_image_url: Option<String>,
    pub page_count: Option<i32>,
    pub publisher: Option<String>,
    pub average_rating: Option<f64>,
    pub preview_link: Option<String>,
}

#[derive(Debug)]
pub enum WorkError {
    InvalidIsbn(String),
    Duplicate { work_id: String },
    MessagingError(String),
    DatabaseError(String),
}

impl std::fmt::Display for WorkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkError::InvalidIsbn(msg) => write!(f, "{msg}"),
            WorkError::Duplicate { work_id } => {
                write!(f, "duplicate ISBN, existing work: {work_id}")
            }
            WorkError::MessagingError(msg) => write!(f, "messaging error: {msg}"),
            WorkError::DatabaseError(msg) => write!(f, "database error: {msg}"),
        }
    }
}

/// Validates and normalises an ISBN-10 or ISBN-13 string.
/// Hyphens are stripped before validation.
/// Returns the normalised digit string on success.
pub fn validate_isbn(isbn: &str) -> Result<String, WorkError> {
    let cleaned: String = isbn.chars().filter(|c| *c != '-').collect();

    match cleaned.len() {
        13 => {
            if !cleaned.chars().all(|c| c.is_ascii_digit()) {
                return Err(WorkError::InvalidIsbn(
                    "Invalid ISBN-13 — must contain only digits".to_string(),
                ));
            }
            validate_isbn13(&cleaned)?;
            Ok(cleaned)
        }
        10 => {
            validate_isbn10(&cleaned)?;
            Ok(cleaned)
        }
        n => Err(WorkError::InvalidIsbn(format!(
            "Invalid ISBN — expected 10 or 13 digits, got {n}"
        ))),
    }
}

fn validate_isbn13(isbn: &str) -> Result<(), WorkError> {
    let sum: u32 = isbn
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let d = c.to_digit(10).unwrap();
            if i % 2 == 0 {
                d
            } else {
                d * 3
            }
        })
        .sum();

    if !sum.is_multiple_of(10) {
        return Err(WorkError::InvalidIsbn(
            "Invalid ISBN-13 — check digit mismatch".to_string(),
        ));
    }
    Ok(())
}

fn validate_isbn10(isbn: &str) -> Result<(), WorkError> {
    let chars: Vec<char> = isbn.chars().collect();

    for c in chars.iter().take(9) {
        if !c.is_ascii_digit() {
            return Err(WorkError::InvalidIsbn(
                "Invalid ISBN-10 — non-digit in positions 1–9".to_string(),
            ));
        }
    }
    if !chars[9].is_ascii_digit() && chars[9] != 'X' {
        return Err(WorkError::InvalidIsbn(
            "Invalid ISBN-10 — invalid check character".to_string(),
        ));
    }

    let sum: u32 = chars
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let d = if c == 'X' {
                10
            } else {
                c.to_digit(10).unwrap()
            };
            d * (10 - i as u32)
        })
        .sum();

    if !sum.is_multiple_of(11) {
        return Err(WorkError::InvalidIsbn(
            "Invalid ISBN-10 — check digit mismatch".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ISBN-13 happy path
    #[test]
    fn valid_isbn13_is_accepted() {
        // 9780132350884 — Clean Code by Robert C. Martin
        assert!(validate_isbn("9780132350884").is_ok());
    }

    #[test]
    fn valid_isbn13_with_hyphens_is_accepted() {
        assert!(validate_isbn("978-0-13-235088-4").is_ok());
    }

    #[test]
    fn isbn13_check_digit_mismatch_is_rejected() {
        // Change last digit to make check fail
        let err = validate_isbn("9780132350885").unwrap_err();
        assert!(err.to_string().contains("check digit mismatch"));
    }

    #[test]
    fn isbn13_with_non_digits_is_rejected() {
        let err = validate_isbn("978013235088X").unwrap_err();
        assert!(err.to_string().contains("only digits"));
    }

    // ISBN-10 happy path
    #[test]
    fn valid_isbn10_is_accepted() {
        // 0132350882 — Clean Code
        assert!(validate_isbn("0132350882").is_ok());
    }

    #[test]
    fn valid_isbn10_with_x_check_digit_is_accepted() {
        // 080442957X — a known ISBN-10 with X check digit
        assert!(validate_isbn("080442957X").is_ok());
    }

    #[test]
    fn valid_isbn10_with_hyphens_is_accepted() {
        assert!(validate_isbn("0-13-235088-2").is_ok());
    }

    #[test]
    fn isbn10_check_digit_mismatch_is_rejected() {
        let err = validate_isbn("0132350883").unwrap_err();
        assert!(err.to_string().contains("check digit mismatch"));
    }

    #[test]
    fn isbn10_invalid_check_char_is_rejected() {
        let err = validate_isbn("013235088Y").unwrap_err();
        assert!(err.to_string().contains("invalid check character"));
    }

    // Wrong length
    #[test]
    fn too_short_isbn_is_rejected() {
        let err = validate_isbn("123456").unwrap_err();
        assert!(err.to_string().contains("expected 10 or 13"));
    }

    #[test]
    fn eleven_digit_isbn_is_rejected() {
        let err = validate_isbn("12345678901").unwrap_err();
        assert!(err.to_string().contains("expected 10 or 13"));
    }
}
