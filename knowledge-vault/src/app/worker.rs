use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::ports::{
    external::{ExternalError, GeminiPort, OpenLibraryPort},
    messaging::{MessageHandler, WorkerError},
    repository::{ConceptRepo, InsightRepo, RepoError, WorkRepo},
};

impl From<RepoError> for WorkerError {
    fn from(e: RepoError) -> Self {
        WorkerError::Transient(e.to_string())
    }
}

impl From<ExternalError> for WorkerError {
    fn from(e: ExternalError) -> Self {
        match e {
            ExternalError::Transient(msg) => WorkerError::Transient(msg),
            ExternalError::Permanent(msg) => WorkerError::Permanent(msg),
        }
    }
}

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

#[derive(Debug, Deserialize)]
struct DiscoveryMessage {
    work_id: String,
    identifier: String,
    #[allow(dead_code)]
    identifier_type: String,
}

pub struct WorkerService {
    work_repo: Arc<dyn WorkRepo>,
    insight_repo: Arc<dyn InsightRepo>,
    concept_repo: Arc<dyn ConceptRepo>,
    openlib: Arc<dyn OpenLibraryPort>,
    gemini: Arc<dyn GeminiPort>,
}

impl WorkerService {
    pub fn new(
        work_repo: Arc<dyn WorkRepo>,
        insight_repo: Arc<dyn InsightRepo>,
        concept_repo: Arc<dyn ConceptRepo>,
        openlib: Arc<dyn OpenLibraryPort>,
        gemini: Arc<dyn GeminiPort>,
    ) -> Self {
        Self {
            work_repo,
            insight_repo,
            concept_repo,
            openlib,
            gemini,
        }
    }

    pub fn spawn(self: Arc<Self>, consumer: crate::adapters::nats::consumer::NatsConsumer) {
        tokio::spawn(async move {
            if let Err(e) = consumer.run(self.as_ref()).await {
                tracing::error!("worker loop exited with error: {e}");
            }
        });
    }

    async fn process(&self, payload: &[u8]) -> Result<(), WorkerError> {
        let text = std::str::from_utf8(payload)
            .map_err(|e| WorkerError::Permanent(format!("invalid UTF-8 in message: {e}")))?;

        let dm: DiscoveryMessage = serde_json::from_str(text)
            .map_err(|e| WorkerError::Permanent(format!("invalid message JSON: {e}")))?;

        tracing::info!(work_id = %dm.work_id, isbn = %dm.identifier, "processing discovery message");

        self.work_repo
            .update_status(&dm.work_id, "processing", None)
            .await?;

        let metadata = self.openlib.fetch_by_isbn(&dm.identifier).await?;

        let gemini_resp = self.gemini.extract_concepts(&metadata).await?;

        let raw_json = serde_json::to_string(&gemini_resp).map_err(|e| {
            WorkerError::Permanent(format!("failed to serialize gemini response: {e}"))
        })?;

        let insight_id = self
            .insight_repo
            .create_insight(
                &dm.work_id,
                &gemini_resp.summary,
                gemini_resp.key_points.clone(),
                &raw_json,
            )
            .await?;

        self.concept_repo
            .upsert_and_link(&dm.work_id, &insight_id, gemini_resp.concepts)
            .await?;

        self.work_repo
            .update_status(&dm.work_id, "done", None)
            .await?;

        tracing::info!(work_id = %dm.work_id, "work processing complete");

        Ok(())
    }
}

#[async_trait]
impl MessageHandler for WorkerService {
    async fn handle(&self, _subject: &str, payload: &[u8]) -> Result<(), WorkerError> {
        self.process(payload).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::messaging::{backoff_delay, should_retry, MAX_ATTEMPTS};
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
