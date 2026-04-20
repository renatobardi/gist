use crate::ports::messaging::WorkerError;

/// Classifies a raw error string into a worker error kind.
///
/// Transient errors are network/timeout/service-unavailable conditions that
/// may succeed on a subsequent attempt. Permanent errors indicate a data or
/// logic problem that will not resolve through retrying.
pub fn classify_error(msg: &str) -> WorkerError {
    let lower = msg.to_lowercase();
    if lower.contains("timeout")
        || lower.contains("connection refused")
        || lower.contains("service unavailable")
        || lower.contains("too many requests")
        || lower.contains("rate limit")
    {
        WorkerError::Transient(msg.to_string())
    } else {
        WorkerError::Permanent(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::messaging::{backoff_delay, should_retry, WorkerError, MAX_ATTEMPTS};
    use std::time::Duration;

    #[test]
    fn timeout_is_classified_as_transient() {
        assert_eq!(
            classify_error("connection timeout after 15s"),
            WorkerError::Transient("connection timeout after 15s".into())
        );
    }

    #[test]
    fn connection_refused_is_classified_as_transient() {
        assert_eq!(
            classify_error("Connection refused (os error 111)"),
            WorkerError::Transient("Connection refused (os error 111)".into())
        );
    }

    #[test]
    fn service_unavailable_is_classified_as_transient() {
        assert_eq!(
            classify_error("service unavailable: upstream not ready"),
            WorkerError::Transient("service unavailable: upstream not ready".into())
        );
    }

    #[test]
    fn too_many_requests_is_classified_as_transient() {
        assert_eq!(
            classify_error("too many requests: quota exceeded"),
            WorkerError::Transient("too many requests: quota exceeded".into())
        );
    }

    #[test]
    fn schema_violation_is_classified_as_permanent() {
        assert_eq!(
            classify_error("missing required field: name"),
            WorkerError::Permanent("missing required field: name".into())
        );
    }

    #[test]
    fn full_retry_sequence_exhausts_at_five_attempts() {
        // Simulate the retry state machine over 5 deliveries.
        let err = WorkerError::Transient("timeout".into());
        let mut retry_count = 0u32;
        let mut terminate_at = 0u32;

        for delivered in 1..=10 {
            if should_retry(delivered, &err) {
                retry_count += 1;
            } else {
                terminate_at = delivered;
                break;
            }
        }

        assert_eq!(retry_count, MAX_ATTEMPTS - 1);
        assert_eq!(terminate_at, MAX_ATTEMPTS);
    }

    #[test]
    fn backoff_delays_grow_exponentially_for_five_attempts() {
        let expected = [
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(4),
            Duration::from_secs(8),
        ];
        for (i, &expected_delay) in expected.iter().enumerate() {
            let delivered = (i + 1) as u32;
            assert_eq!(backoff_delay(delivered), expected_delay);
        }
    }
}
