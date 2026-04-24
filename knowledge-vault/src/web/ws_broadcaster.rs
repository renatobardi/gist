use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub struct WsBroadcaster {
    tx: broadcast::Sender<String>,
}

impl WsBroadcaster {
    pub fn new() -> Arc<Self> {
        let (tx, _) = broadcast::channel(256);
        Arc::new(Self { tx })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    pub fn broadcast(&self, work_id: &str, status: &str) {
        let msg = serde_json::json!({
            "type": "work_status",
            "work_id": work_id,
            "status": status,
        })
        .to_string();
        let _ = self.tx.send(msg);
    }

    pub fn broadcast_progress(&self, work_id: &str, progress_pct: i32, last_action: &str) {
        let msg = serde_json::json!({
            "type": "work_progress",
            "work_id": work_id,
            "progress_pct": progress_pct,
            "last_action": last_action,
        })
        .to_string();
        let _ = self.tx.send(msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn broadcast_delivers_to_subscriber() {
        let broadcaster = WsBroadcaster::new();
        let mut rx = broadcaster.subscribe();

        broadcaster.broadcast("work-1", "processing");

        let msg = rx.recv().await.unwrap();
        let v: serde_json::Value = serde_json::from_str(&msg).unwrap();
        assert_eq!(v["type"], "work_status");
        assert_eq!(v["work_id"], "work-1");
        assert_eq!(v["status"], "processing");
    }

    #[tokio::test]
    async fn subscriber_after_broadcast_does_not_receive_stale_message() {
        let broadcaster = WsBroadcaster::new();

        broadcaster.broadcast("work-1", "done");

        let mut rx = broadcaster.subscribe();
        broadcaster.broadcast("work-2", "failed");

        let msg = rx.recv().await.unwrap();
        let v: serde_json::Value = serde_json::from_str(&msg).unwrap();
        assert_eq!(v["work_id"], "work-2");
    }

    #[tokio::test]
    async fn broadcast_with_no_subscribers_does_not_panic() {
        let broadcaster = WsBroadcaster::new();
        broadcaster.broadcast("work-1", "done");
    }

    #[tokio::test]
    async fn broadcast_progress_delivers_pct_and_action_to_subscriber() {
        let broadcaster = WsBroadcaster::new();
        let mut rx = broadcaster.subscribe();

        broadcaster.broadcast_progress("work-1", 50, "Extracting concepts");

        let msg = rx.recv().await.unwrap();
        let v: serde_json::Value = serde_json::from_str(&msg).unwrap();
        assert_eq!(v["type"], "work_progress");
        assert_eq!(v["work_id"], "work-1");
        assert_eq!(v["progress_pct"], 50);
        assert_eq!(v["last_action"], "Extracting concepts");
    }
}
