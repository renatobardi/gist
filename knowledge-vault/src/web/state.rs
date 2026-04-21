use std::sync::Arc;

use crate::ports::external::OpenLibraryPort;
use crate::ports::messaging::MessagePublisher;
use crate::ports::repository::{LoginAttemptRepo, TokenRepo, UserRepo, WorkRepo};
use crate::web::ws_broadcaster::WsBroadcaster;

#[derive(Clone)]
pub struct AppState {
    pub user_repo: Arc<dyn UserRepo>,
    pub login_attempt_repo: Arc<dyn LoginAttemptRepo>,
    pub token_repo: Arc<dyn TokenRepo>,
    pub work_repo: Arc<dyn WorkRepo>,
    pub message_publisher: Option<Arc<dyn MessagePublisher>>,
    pub open_library_client: Option<Arc<dyn OpenLibraryPort>>,
    pub ws_broadcaster: Arc<WsBroadcaster>,
    pub jwt_secret: String,
}
