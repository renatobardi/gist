/// Integration tests for the NATS consumer retry policy.
///
/// These tests require a running NATS server with JetStream enabled on
/// localhost:4222. They are marked `#[ignore]` so they are skipped in CI
/// (which does not provision a NATS server). Run with:
///   cargo test -- --ignored
use async_nats::jetstream::{
    self, consumer::pull::Config as PullConfig, stream::Config as StreamConfig,
};
use async_trait::async_trait;
use knowledge_vault::{
    adapters::nats::consumer::NatsConsumer,
    ports::messaging::{MessageHandler, WorkerError, MAX_ATTEMPTS},
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

async fn connect_jetstream() -> jetstream::Context {
    let client = async_nats::connect("nats://127.0.0.1:4222")
        .await
        .expect("NATS must be running on localhost:4222 for integration tests");
    jetstream::new(client)
}

struct CountingHandler {
    call_count: Arc<AtomicU32>,
    fail_until: u32,
    failure_kind: &'static str,
}

#[async_trait]
impl MessageHandler for CountingHandler {
    async fn handle(&self, _subject: &str, _payload: &[u8]) -> Result<(), WorkerError> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst) + 1;
        if count < self.fail_until {
            match self.failure_kind {
                "transient" => Err(WorkerError::Transient(format!("attempt {count} failed"))),
                _ => Err(WorkerError::Permanent(format!(
                    "permanent on attempt {count}"
                ))),
            }
        } else {
            Ok(())
        }
    }
}

struct AlwaysTransientHandler {
    call_count: Arc<AtomicU32>,
}

#[async_trait]
impl MessageHandler for AlwaysTransientHandler {
    async fn handle(&self, _subject: &str, _payload: &[u8]) -> Result<(), WorkerError> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst) + 1;
        Err(WorkerError::Transient(format!(
            "always fails, attempt {count}"
        )))
    }
}

#[tokio::test]
#[ignore = "requires NATS server on localhost:4222"]
async fn transient_failure_retries_and_succeeds_on_third_attempt() {
    let js = connect_jetstream().await;
    let stream_name = "TEST_RETRY_SUCCESS";
    let subject = "test.retry.success";

    let _ = js.delete_stream(stream_name).await;
    js.create_stream(StreamConfig {
        name: stream_name.to_string(),
        subjects: vec![subject.to_string()],
        max_age: Duration::from_secs(60),
        ..Default::default()
    })
    .await
    .unwrap();

    js.publish(subject, b"hello".as_ref().into()).await.unwrap();

    let stream = js.get_stream(stream_name).await.unwrap();
    let consumer = stream
        .create_consumer(PullConfig {
            name: Some("retry_success_consumer".to_string()),
            ack_wait: Duration::from_secs(5),
            max_deliver: MAX_ATTEMPTS as i64,
            ..Default::default()
        })
        .await
        .unwrap();

    let nats_consumer = NatsConsumer::new(consumer);
    let call_count = Arc::new(AtomicU32::new(0));
    let handler = CountingHandler {
        call_count: Arc::clone(&call_count),
        fail_until: 3,
        failure_kind: "transient",
    };

    tokio::time::timeout(Duration::from_secs(30), nats_consumer.run(&handler))
        .await
        .ok();

    assert_eq!(call_count.load(Ordering::SeqCst), 3);

    js.delete_stream(stream_name).await.unwrap();
}

#[tokio::test]
#[ignore = "requires NATS server on localhost:4222"]
async fn permanent_failure_terminates_without_retry() {
    let js = connect_jetstream().await;
    let stream_name = "TEST_PERM_FAIL";
    let subject = "test.perm.fail";

    let _ = js.delete_stream(stream_name).await;
    js.create_stream(StreamConfig {
        name: stream_name.to_string(),
        subjects: vec![subject.to_string()],
        max_age: Duration::from_secs(60),
        ..Default::default()
    })
    .await
    .unwrap();

    js.publish(subject, b"bad data".as_ref().into())
        .await
        .unwrap();

    let stream = js.get_stream(stream_name).await.unwrap();
    let consumer = stream
        .create_consumer(PullConfig {
            name: Some("perm_fail_consumer".to_string()),
            ack_wait: Duration::from_secs(5),
            max_deliver: MAX_ATTEMPTS as i64,
            ..Default::default()
        })
        .await
        .unwrap();

    let nats_consumer = NatsConsumer::new(consumer);
    let call_count = Arc::new(AtomicU32::new(0));
    let handler = CountingHandler {
        call_count: Arc::clone(&call_count),
        fail_until: u32::MAX,
        failure_kind: "permanent",
    };

    tokio::time::timeout(Duration::from_secs(10), nats_consumer.run(&handler))
        .await
        .ok();

    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "permanent failure must not trigger any retries"
    );

    js.delete_stream(stream_name).await.unwrap();
}

#[tokio::test]
#[ignore = "requires NATS server on localhost:4222"]
async fn transient_failure_exhausts_all_five_attempts_then_terminates() {
    let js = connect_jetstream().await;
    let stream_name = "TEST_MAX_RETRY";
    let subject = "test.max.retry";

    let _ = js.delete_stream(stream_name).await;
    js.create_stream(StreamConfig {
        name: stream_name.to_string(),
        subjects: vec![subject.to_string()],
        max_age: Duration::from_secs(120),
        ..Default::default()
    })
    .await
    .unwrap();

    js.publish(subject, b"will exhaust".as_ref().into())
        .await
        .unwrap();

    let stream = js.get_stream(stream_name).await.unwrap();
    let consumer = stream
        .create_consumer(PullConfig {
            name: Some("max_retry_consumer".to_string()),
            ack_wait: Duration::from_secs(2),
            max_deliver: MAX_ATTEMPTS as i64,
            // Short backoff override not possible via PullConfig; NATS server
            // respects the NAK delay sent by the client.
            ..Default::default()
        })
        .await
        .unwrap();

    let nats_consumer = NatsConsumer::new(consumer);
    let call_count = Arc::new(AtomicU32::new(0));
    let handler = AlwaysTransientHandler {
        call_count: Arc::clone(&call_count),
    };

    // Generous timeout: 1+2+4+8 = 15s of backoff + processing overhead
    tokio::time::timeout(Duration::from_secs(60), nats_consumer.run(&handler))
        .await
        .ok();

    assert_eq!(
        call_count.load(Ordering::SeqCst),
        MAX_ATTEMPTS,
        "transient failure must be attempted exactly MAX_ATTEMPTS ({MAX_ATTEMPTS}) times"
    );

    js.delete_stream(stream_name).await.unwrap();
}
