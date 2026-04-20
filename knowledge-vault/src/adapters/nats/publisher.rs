use async_nats::Client;
use async_trait::async_trait;

use crate::ports::messaging::MessagePublisher;

pub struct NatsPublisher {
    client: Client,
}

impl NatsPublisher {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl MessagePublisher for NatsPublisher {
    async fn publish(&self, subject: &str, payload: Vec<u8>) -> Result<(), String> {
        self.client
            .publish(subject.to_string(), payload.into())
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
