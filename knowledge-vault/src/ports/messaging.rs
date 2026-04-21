use async_trait::async_trait;
use std::time::Duration;

/// Maximum number of delivery attempts before a message is permanently terminated.
pub const MAX_ATTEMPTS: u32 = 5;

/// Classifies a worker processing error as transient (retryable) or permanent (fail immediately).
#[derive(Debug, Clone, PartialEq)]
pub enum WorkerError {
    Transient(String),
    Permanent(String),
}

impl std::fmt::Display for WorkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerError::Transient(msg) => write!(f, "transient: {msg}"),
            WorkerError::Permanent(msg) => write!(f, "permanent: {msg}"),
        }
    }
}

/// Returns the exponential backoff delay for a given delivery count.
///
/// `delivered` is 1-indexed (1 = first delivery, 2 = first retry, ...).
/// Sequence: 1s → 2s → 4s → 8s, capped at 30s.
pub fn backoff_delay(delivered: u32) -> Duration {
    let secs = 1u64
        .checked_shl(delivered.saturating_sub(1))
        .unwrap_or(u64::MAX);
    Duration::from_secs(secs.min(30))
}

/// Returns true if the message should be retried given its delivery count and error.
///
/// Retries only for transient errors and only when `delivered < MAX_ATTEMPTS`.
pub fn should_retry(delivered: u32, error: &WorkerError) -> bool {
    matches!(error, WorkerError::Transient(_)) && delivered < MAX_ATTEMPTS
}

#[async_trait]
pub trait MessagePublisher: Send + Sync {
    async fn publish(&self, subject: &str, payload: Vec<u8>) -> Result<(), String>;
}

#[async_trait]
pub trait MessageHandler: Send + Sync {
    async fn handle(&self, subject: &str, payload: &[u8]) -> Result<(), WorkerError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_error_retries_below_max_attempts() {
        let err = WorkerError::Transient("timeout".into());
        assert!(should_retry(1, &err));
        assert!(should_retry(2, &err));
        assert!(should_retry(3, &err));
        assert!(should_retry(4, &err));
    }

    #[test]
    fn transient_error_does_not_retry_at_max_attempts() {
        let err = WorkerError::Transient("timeout".into());
        assert!(!should_retry(MAX_ATTEMPTS, &err));
    }

    #[test]
    fn transient_error_does_not_retry_beyond_max_attempts() {
        let err = WorkerError::Transient("timeout".into());
        assert!(!should_retry(MAX_ATTEMPTS + 1, &err));
    }

    #[test]
    fn permanent_error_is_never_retried() {
        let err = WorkerError::Permanent("schema violation".into());
        assert!(!should_retry(1, &err));
        assert!(!should_retry(3, &err));
        assert!(!should_retry(MAX_ATTEMPTS - 1, &err));
    }

    #[test]
    fn retry_count_matches_max_attempts_minus_one() {
        // Delivered 1..4 can retry; delivered 5 terminates. Total attempts = 5.
        let err = WorkerError::Transient("timeout".into());
        let retryable: u32 = (1..=MAX_ATTEMPTS)
            .filter(|&d| should_retry(d, &err))
            .count() as u32;
        assert_eq!(retryable, MAX_ATTEMPTS - 1);
    }

    #[test]
    fn backoff_delay_sequence_is_exponential() {
        assert_eq!(backoff_delay(1), Duration::from_secs(1));
        assert_eq!(backoff_delay(2), Duration::from_secs(2));
        assert_eq!(backoff_delay(3), Duration::from_secs(4));
        assert_eq!(backoff_delay(4), Duration::from_secs(8));
    }

    #[test]
    fn backoff_delay_is_capped_at_30s() {
        assert_eq!(backoff_delay(6), Duration::from_secs(30));
        assert_eq!(backoff_delay(100), Duration::from_secs(30));
    }
}
