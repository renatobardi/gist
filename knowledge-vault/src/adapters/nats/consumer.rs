use async_nats::jetstream::{
    consumer::{pull::Config as PullConfig, Consumer},
    AckKind, Message,
};
use futures::StreamExt;
use tracing::{error, info, warn};

use crate::ports::messaging::{backoff_delay, should_retry, MessageHandler, WorkerError};

pub struct NatsConsumer {
    consumer: Consumer<PullConfig>,
}

impl NatsConsumer {
    pub fn new(consumer: Consumer<PullConfig>) -> Self {
        Self { consumer }
    }

    /// Runs the consumer loop: pulls messages and dispatches them to `handler`.
    ///
    /// For each message:
    /// - On success: acks the message.
    /// - On permanent error: terminates delivery immediately (no further retries).
    /// - On transient error with retries remaining: nacks with exponential backoff delay.
    /// - On transient error at max attempts: terminates delivery and logs an error.
    pub async fn run<H: MessageHandler>(&self, handler: &H) -> Result<(), String> {
        let mut messages = self
            .consumer
            .messages()
            .await
            .map_err(|e| format!("failed to start message stream: {e}"))?;

        while let Some(msg_result) = messages.next().await {
            let msg: Message = match msg_result {
                Ok(m) => m,
                Err(e) => {
                    error!(error = %e, "error receiving message from NATS");
                    continue;
                }
            };

            let delivered = match msg.info() {
                Ok(info) => info.delivered as u32,
                Err(e) => {
                    error!(error = %e, "failed to read NATS message info — nacking");
                    msg.ack_with(AckKind::Nak(None)).await.ok();
                    continue;
                }
            };

            let subject = msg.subject.as_str().to_string();

            match handler.handle(&subject, &msg.payload).await {
                Ok(()) => {
                    info!(subject, "message processed successfully");
                    msg.ack().await.ok();
                }
                Err(WorkerError::Permanent(e)) => {
                    error!(
                        error = %e,
                        subject,
                        "permanent failure — terminating delivery"
                    );
                    msg.ack_with(AckKind::Term).await.ok();
                }
                Err(WorkerError::Transient(ref e)) => {
                    if should_retry(delivered, &WorkerError::Transient(e.clone())) {
                        let delay = backoff_delay(delivered);
                        warn!(
                            error = %e,
                            subject,
                            attempt = delivered,
                            delay_secs = delay.as_secs(),
                            "transient failure — scheduling retry with backoff"
                        );
                        msg.ack_with(AckKind::Nak(Some(delay))).await.ok();
                    } else {
                        error!(
                            error = %e,
                            subject,
                            attempt = delivered,
                            "max retry attempts reached — terminating delivery"
                        );
                        msg.ack_with(AckKind::Term).await.ok();
                    }
                }
            }
        }

        Ok(())
    }
}
