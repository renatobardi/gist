use std::sync::Arc;

use crate::ports::repository::UserRepo;

#[derive(Clone)]
pub struct AppState {
    pub user_repo: Arc<dyn UserRepo>,
}
