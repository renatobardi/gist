use async_trait::async_trait;

#[async_trait]
pub trait MessagePublisher: Send + Sync {
    async fn publish(&self, subject: &str, payload: Vec<u8>) -> Result<(), String>;
}
