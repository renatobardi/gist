use std::sync::Arc;

use crate::ports::repository::{LoginAttemptRepo, UserRepo};

#[derive(Clone)]
pub struct AppState {
    pub user_repo: Arc<dyn UserRepo>,
    pub login_attempt_repo: Arc<dyn LoginAttemptRepo>,
    pub jwt_secret: String,
}
