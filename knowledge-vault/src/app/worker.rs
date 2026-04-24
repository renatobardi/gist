use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::ports::{
    external::{ExternalError, GeminiPort, GoogleBooksPort, OpenLibraryPort},
    messaging::{MessageHandler, WorkerError},
    repository::{GraphWriteRepo, RepoError, WorkRepo},
};
use crate::web::ws_broadcaster::WsBroadcaster;

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
            ExternalError::SkippedOptional(msg) => WorkerError::Permanent(msg),
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
    identifier_type: String,
}

pub struct WorkerService {
    work_repo: Arc<dyn WorkRepo>,
    graph_write_repo: Arc<dyn GraphWriteRepo>,
    openlib: Arc<dyn OpenLibraryPort>,
    gemini: Arc<dyn GeminiPort>,
    google_books: Option<Arc<dyn GoogleBooksPort>>,
    ws_broadcaster: Arc<WsBroadcaster>,
}

impl WorkerService {
    pub fn new(
        work_repo: Arc<dyn WorkRepo>,
        graph_write_repo: Arc<dyn GraphWriteRepo>,
        openlib: Arc<dyn OpenLibraryPort>,
        gemini: Arc<dyn GeminiPort>,
        google_books: Option<Arc<dyn GoogleBooksPort>>,
        ws_broadcaster: Arc<WsBroadcaster>,
    ) -> Self {
        Self {
            work_repo,
            graph_write_repo,
            openlib,
            gemini,
            google_books,
            ws_broadcaster,
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

        tracing::info!(
            work_id = %dm.work_id,
            identifier = %dm.identifier,
            identifier_type = %dm.identifier_type,
            "processing discovery message"
        );

        self.work_repo
            .update_status(&dm.work_id, "processing", None)
            .await?;
        self.ws_broadcaster.broadcast(&dm.work_id, "processing");

        let result = self.run_pipeline(&dm).await;

        // Persist the failure so the UI reflects the terminal state.
        // Only permanent failures are updated here; transient failures stay as
        // "processing" while the consumer retries with backoff.
        // TODO: when the consumer exhausts max retry attempts it also calls
        // AckKind::Term without reaching here, so the work stays stuck at
        // "processing". Fix requires the consumer to signal terminal exhaustion
        // back to the handler (e.g. an on_terminal_failure callback on
        // MessageHandler) so the status can be persisted.
        if let Err(WorkerError::Permanent(ref e)) = result {
            let _ = self
                .work_repo
                .update_status(&dm.work_id, "failed", Some(e.as_str()))
                .await;
            self.ws_broadcaster.broadcast(&dm.work_id, "failed");
        }

        result
    }

    async fn run_pipeline(&self, dm: &DiscoveryMessage) -> Result<(), WorkerError> {
        // 0%: beginning of pipeline
        self.emit_progress(&dm.work_id, 0, "Fetching metadata from Open Library")
            .await;

        let metadata = if dm.identifier_type == "title" {
            let book = self
                .openlib
                .search_by_title(&dm.identifier)
                .await
                .map_err(WorkerError::Transient)?
                .ok_or_else(|| {
                    WorkerError::Permanent(format!(
                        "title '{}' not found in Open Library",
                        dm.identifier
                    ))
                })?;
            self.openlib.fetch_by_work_id(&book.open_library_id).await?
        } else {
            self.openlib.fetch_by_isbn(&dm.identifier).await?
        };

        // 25%: Open Library metadata fetched
        self.emit_progress(&dm.work_id, 25, "Enriching with Google Books")
            .await;

        // Google Books step: only runs for ISBN submissions; errors are non-fatal
        if let Some(ref gb_client) = self.google_books {
            if dm.identifier_type == "isbn" {
                match gb_client.fetch_by_isbn(&dm.identifier).await {
                    Ok(Some(meta)) => {
                        if let Err(e) = self
                            .work_repo
                            .update_google_books_metadata(
                                &dm.work_id,
                                meta.cover_image_url.as_deref(),
                                meta.page_count.map(|p| p as i32),
                                meta.publisher.as_deref(),
                                meta.average_rating,
                                meta.preview_link.as_deref(),
                            )
                            .await
                        {
                            tracing::warn!(
                                work_id = %dm.work_id,
                                "failed to persist Google Books metadata: {e}"
                            );
                        }
                    }
                    Ok(None) => {
                        tracing::debug!(
                            work_id = %dm.work_id,
                            isbn = %dm.identifier,
                            "no Google Books results for ISBN"
                        );
                    }
                    Err(ExternalError::SkippedOptional(ref msg)) => {
                        tracing::debug!(
                            work_id = %dm.work_id,
                            "Google Books step skipped: {msg}"
                        );
                    }
                    Err(ExternalError::Transient(ref msg)) => {
                        tracing::warn!(
                            work_id = %dm.work_id,
                            "Google Books transient error (non-fatal, continuing): {msg}"
                        );
                    }
                    Err(ExternalError::Permanent(ref msg)) => {
                        tracing::warn!(
                            work_id = %dm.work_id,
                            "Google Books permanent error (non-fatal, continuing): {msg}"
                        );
                    }
                }
            }
        }

        // 50%: Google Books step complete (or skipped)
        self.emit_progress(&dm.work_id, 50, "Extracting concepts with Gemini")
            .await;

        let gemini_resp = self.gemini.extract_concepts(&metadata).await?;

        // 75%: Gemini concepts extracted
        self.emit_progress(&dm.work_id, 75, "Writing to knowledge graph")
            .await;

        // Atomic graph write: insight + edges + concepts + work status = "done" in one transaction.
        self.graph_write_repo
            .write_graph_transaction(&dm.work_id, &gemini_resp)
            .await?;

        // 100%: pipeline complete
        self.emit_progress(&dm.work_id, 100, "Complete").await;
        self.ws_broadcaster.broadcast(&dm.work_id, "done");

        tracing::info!(work_id = %dm.work_id, "work processing complete");

        Ok(())
    }

    async fn emit_progress(&self, work_id: &str, pct: i32, action: &str) {
        if let Err(e) = self.work_repo.update_progress(work_id, pct, action).await {
            tracing::warn!(work_id = %work_id, pct, action, "failed to persist progress: {e}");
        }
        self.ws_broadcaster.broadcast_progress(work_id, pct, action);
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
    use crate::domain::{insight::GeminiResponse, work::Work};
    use crate::ports::{
        external::{
            BookMetadata, ExternalError, GeminiPort, GoogleBooksMetadata, GoogleBooksPort,
            OpenLibraryBook, OpenLibraryPort,
        },
        messaging::{backoff_delay, should_retry, WorkerError, MAX_ATTEMPTS},
        repository::{GraphWriteRepo, RepoError, SortOrder, WorkRepo, WorkSortField},
    };
    use std::sync::Mutex;
    use std::time::Duration;

    // ---- minimal stubs ------------------------------------------------

    struct StubWorkRepo {
        progress_calls: Mutex<Vec<(i32, String)>>,
    }

    impl StubWorkRepo {
        fn new() -> Self {
            Self {
                progress_calls: Mutex::new(vec![]),
            }
        }
    }

    #[async_trait]
    impl WorkRepo for StubWorkRepo {
        async fn find_by_isbn(&self, _: &str) -> Result<Option<Work>, RepoError> {
            unimplemented!()
        }
        async fn find_by_id(&self, _: &str) -> Result<Option<Work>, RepoError> {
            unimplemented!()
        }
        async fn create_work(&self, _: &str) -> Result<Work, RepoError> {
            unimplemented!()
        }
        async fn find_by_open_library_id(&self, _: &str) -> Result<Option<Work>, RepoError> {
            unimplemented!()
        }
        async fn create_work_by_title(&self, _: &str, _: &str, _: &str) -> Result<Work, RepoError> {
            unimplemented!()
        }
        async fn list_works(&self, _: u32, _: u32) -> Result<Vec<Work>, RepoError> {
            unimplemented!()
        }
        async fn get_work_by_id(&self, _: &str) -> Result<Option<Work>, RepoError> {
            unimplemented!()
        }
        async fn update_work_status(
            &self,
            _: &str,
            _: &str,
            _: Option<&str>,
        ) -> Result<(), RepoError> {
            Ok(())
        }
        async fn update_status(&self, _: &str, _: &str, _: Option<&str>) -> Result<(), RepoError> {
            Ok(())
        }
        async fn reset_to_pending(&self, _: &str) -> Result<Work, RepoError> {
            unimplemented!()
        }
        async fn delete_work_cascade(&self, _: &str) -> Result<(), RepoError> {
            unimplemented!()
        }
        async fn update_progress(&self, _: &str, pct: i32, action: &str) -> Result<(), RepoError> {
            self.progress_calls
                .lock()
                .unwrap()
                .push((pct, action.to_string()));
            Ok(())
        }
        async fn update_google_books_metadata(
            &self,
            _: &str,
            _: Option<&str>,
            _: Option<i32>,
            _: Option<&str>,
            _: Option<f64>,
            _: Option<&str>,
        ) -> Result<(), RepoError> {
            Ok(())
        }
        async fn update_reading_status(&self, _: &str, _: Option<&str>) -> Result<Work, RepoError> {
            unimplemented!()
        }
        async fn list_works_filtered(
            &self,
            _: Option<&str>,
            _: Option<&str>,
            _: WorkSortField,
            _: SortOrder,
            _: u32,
            _: u32,
        ) -> Result<Vec<Work>, RepoError> {
            unimplemented!()
        }
    }

    struct StubGraphWriteRepo;

    #[async_trait]
    impl GraphWriteRepo for StubGraphWriteRepo {
        async fn write_graph_transaction(
            &self,
            _: &str,
            _: &GeminiResponse,
        ) -> Result<(), RepoError> {
            Ok(())
        }
    }

    struct StubGemini;

    #[async_trait]
    impl GeminiPort for StubGemini {
        async fn extract_concepts(
            &self,
            _: &BookMetadata,
        ) -> Result<GeminiResponse, ExternalError> {
            Ok(GeminiResponse {
                summary: "stub".into(),
                key_points: vec![],
                concepts: vec![],
            })
        }
    }

    struct StubGoogleBooks {
        result: Mutex<Option<Result<Option<GoogleBooksMetadata>, ExternalError>>>,
    }

    impl StubGoogleBooks {
        fn returns_none() -> Arc<Self> {
            Arc::new(Self {
                result: Mutex::new(Some(Ok(None))),
            })
        }

        fn returns_skipped() -> Arc<Self> {
            Arc::new(Self {
                result: Mutex::new(Some(Err(ExternalError::SkippedOptional(
                    "no api key".into(),
                )))),
            })
        }

        fn returns_transient_error() -> Arc<Self> {
            Arc::new(Self {
                result: Mutex::new(Some(Err(ExternalError::Transient(
                    "connection timeout".into(),
                )))),
            })
        }

        fn returns_permanent_error() -> Arc<Self> {
            Arc::new(Self {
                result: Mutex::new(Some(Err(ExternalError::Permanent(
                    "invalid API key".into(),
                )))),
            })
        }
    }

    #[async_trait]
    impl GoogleBooksPort for StubGoogleBooks {
        async fn fetch_by_isbn(
            &self,
            _isbn: &str,
        ) -> Result<Option<GoogleBooksMetadata>, ExternalError> {
            self.result
                .lock()
                .unwrap()
                .take()
                .unwrap_or(Ok(None))
        }
    }

    // ---- routing mock: records which OpenLib method was called --------

    struct RoutingMock {
        isbn_calls: Mutex<Vec<String>>,
        work_id_calls: Mutex<Vec<String>>,
    }

    impl RoutingMock {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                isbn_calls: Mutex::new(vec![]),
                work_id_calls: Mutex::new(vec![]),
            })
        }
    }

    #[async_trait]
    impl OpenLibraryPort for RoutingMock {
        async fn search_by_title(&self, title: &str) -> Result<Option<OpenLibraryBook>, String> {
            Ok(Some(OpenLibraryBook {
                open_library_id: "/works/OL123W".to_string(),
                title: title.to_string(),
                author: "Author".to_string(),
            }))
        }

        async fn fetch_by_isbn(&self, isbn: &str) -> Result<BookMetadata, ExternalError> {
            self.isbn_calls.lock().unwrap().push(isbn.to_string());
            Ok(stub_metadata())
        }

        async fn fetch_by_work_id(&self, work_id: &str) -> Result<BookMetadata, ExternalError> {
            self.work_id_calls.lock().unwrap().push(work_id.to_string());
            Ok(stub_metadata())
        }
    }

    fn stub_metadata() -> BookMetadata {
        BookMetadata {
            title: "Book".into(),
            author: "Author".into(),
            description: String::new(),
            subjects: vec![],
        }
    }

    fn make_worker(openlib: Arc<dyn OpenLibraryPort>) -> (WorkerService, Arc<StubWorkRepo>) {
        let repo = Arc::new(StubWorkRepo::new());
        let worker = WorkerService::new(
            repo.clone(),
            Arc::new(StubGraphWriteRepo),
            openlib,
            Arc::new(StubGemini),
            None,
            WsBroadcaster::new(),
        );
        (worker, repo)
    }

    fn make_worker_with_google_books(
        openlib: Arc<dyn OpenLibraryPort>,
        gb: Option<Arc<dyn GoogleBooksPort>>,
    ) -> (WorkerService, Arc<StubWorkRepo>) {
        let repo = Arc::new(StubWorkRepo::new());
        let worker = WorkerService::new(
            repo.clone(),
            Arc::new(StubGraphWriteRepo),
            openlib,
            Arc::new(StubGemini),
            gb,
            WsBroadcaster::new(),
        );
        (worker, repo)
    }

    fn payload(work_id: &str, identifier: &str, identifier_type: &str) -> Vec<u8> {
        serde_json::json!({
            "work_id": work_id,
            "identifier": identifier,
            "identifier_type": identifier_type,
        })
        .to_string()
        .into_bytes()
    }

    #[tokio::test]
    async fn title_submission_routes_to_fetch_by_work_id() {
        let mock = RoutingMock::new();
        let (worker, _) = make_worker(mock.clone());

        worker
            .process(&payload("w1", "Clean Code", "title"))
            .await
            .unwrap();

        assert!(
            mock.isbn_calls.lock().unwrap().is_empty(),
            "fetch_by_isbn must not be called for title submissions"
        );
        assert_eq!(
            mock.work_id_calls.lock().unwrap().as_slice(),
            ["/works/OL123W"],
            "fetch_by_work_id must be called with the work ID returned by search_by_title"
        );
    }

    #[tokio::test]
    async fn isbn_submission_routes_to_fetch_by_isbn() {
        let mock = RoutingMock::new();
        let (worker, _) = make_worker(mock.clone());

        worker
            .process(&payload("w2", "9780132350884", "isbn"))
            .await
            .unwrap();

        assert_eq!(
            mock.isbn_calls.lock().unwrap().as_slice(),
            ["9780132350884"],
            "fetch_by_isbn must be called with the raw ISBN"
        );
        assert!(
            mock.work_id_calls.lock().unwrap().is_empty(),
            "fetch_by_work_id must not be called for isbn submissions"
        );
    }

    #[tokio::test]
    async fn progress_checkpoints_emitted_in_order() {
        let mock = RoutingMock::new();
        let (worker, repo) = make_worker(mock);

        worker
            .process(&payload("w3", "9780132350884", "isbn"))
            .await
            .unwrap();

        let calls = repo.progress_calls.lock().unwrap();
        let pcts: Vec<i32> = calls.iter().map(|(p, _)| *p).collect();
        assert_eq!(pcts, vec![0, 25, 50, 75, 100], "checkpoints must be 0/25/50/75/100 in order");
    }

    #[tokio::test]
    async fn google_books_skipped_when_no_client() {
        let mock = RoutingMock::new();
        let (worker, repo) = make_worker_with_google_books(mock, None);

        worker
            .process(&payload("w4", "9780132350884", "isbn"))
            .await
            .unwrap();

        let calls = repo.progress_calls.lock().unwrap();
        let pcts: Vec<i32> = calls.iter().map(|(p, _)| *p).collect();
        assert_eq!(pcts, vec![0, 25, 50, 75, 100]);
    }

    #[tokio::test]
    async fn google_books_skipped_optional_does_not_fail_pipeline() {
        let mock = RoutingMock::new();
        let gb = StubGoogleBooks::returns_skipped();
        let (worker, repo) =
            make_worker_with_google_books(mock, Some(gb as Arc<dyn GoogleBooksPort>));

        let result = worker
            .process(&payload("w5", "9780132350884", "isbn"))
            .await;

        assert!(result.is_ok(), "SkippedOptional must not abort the pipeline");
        let calls = repo.progress_calls.lock().unwrap();
        let pcts: Vec<i32> = calls.iter().map(|(p, _)| *p).collect();
        assert_eq!(pcts, vec![0, 25, 50, 75, 100]);
    }

    #[tokio::test]
    async fn google_books_not_called_for_title_submissions() {
        let mock = RoutingMock::new();
        let gb = StubGoogleBooks::returns_none();
        let gb_arc = gb.clone() as Arc<dyn GoogleBooksPort>;
        let (worker, _) = make_worker_with_google_books(mock, Some(gb_arc));

        worker
            .process(&payload("w6", "Clean Code", "title"))
            .await
            .unwrap();

        // For title submissions there is no ISBN, so Google Books must not be called.
        // The stub's result slot is still Some (unconsumed) when Google Books is skipped.
        assert!(
            gb.result.lock().unwrap().is_some(),
            "fetch_by_isbn must not be called for title submissions"
        );
    }

    #[tokio::test]
    async fn google_books_transient_error_does_not_fail_pipeline() {
        let mock = RoutingMock::new();
        let gb = StubGoogleBooks::returns_transient_error();
        let (worker, repo) =
            make_worker_with_google_books(mock, Some(gb as Arc<dyn GoogleBooksPort>));

        let result = worker
            .process(&payload("w7", "9780132350884", "isbn"))
            .await;

        assert!(result.is_ok(), "Transient Google Books error must not abort the pipeline");
        let calls = repo.progress_calls.lock().unwrap();
        let pcts: Vec<i32> = calls.iter().map(|(p, _)| *p).collect();
        assert_eq!(pcts, vec![0, 25, 50, 75, 100]);
    }

    #[tokio::test]
    async fn google_books_permanent_error_does_not_fail_pipeline() {
        let mock = RoutingMock::new();
        let gb = StubGoogleBooks::returns_permanent_error();
        let (worker, repo) =
            make_worker_with_google_books(mock, Some(gb as Arc<dyn GoogleBooksPort>));

        let result = worker
            .process(&payload("w8", "9780132350884", "isbn"))
            .await;

        assert!(result.is_ok(), "Permanent Google Books error must not abort the pipeline");
        let calls = repo.progress_calls.lock().unwrap();
        let pcts: Vec<i32> = calls.iter().map(|(p, _)| *p).collect();
        assert_eq!(pcts, vec![0, 25, 50, 75, 100]);
    }

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
