use std::sync::Arc;

use crate::ports::repository::{LoginAttemptRepo, TokenRepo, UserRepo};

#[derive(Clone)]
pub struct AppState {
    pub user_repo: Arc<dyn UserRepo>,
    pub login_attempt_repo: Arc<dyn LoginAttemptRepo>,
    pub token_repo: Arc<dyn TokenRepo>,
    pub jwt_secret: String,
}
