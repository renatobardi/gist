use std::sync::Arc;

use crate::ports::messaging::MessagePublisher;
use crate::ports::repository::{LoginAttemptRepo, TokenRepo, UserRepo, WorkRepo};

#[derive(Clone)]
pub struct AppState {
    pub user_repo: Arc<dyn UserRepo>,
    pub login_attempt_repo: Arc<dyn LoginAttemptRepo>,
    pub token_repo: Arc<dyn TokenRepo>,
    pub work_repo: Arc<dyn WorkRepo>,
    pub message_publisher: Option<Arc<dyn MessagePublisher>>,
    pub jwt_secret: String,
}
