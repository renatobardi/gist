use std::sync::Arc;

use surrealdb::{engine::local::Db, Surreal};

use crate::ports::external::{GoogleBooksPort, OpenLibraryPort};
use crate::ports::messaging::MessagePublisher;
use crate::ports::repository::{
    ConceptRepo, GraphReadRepo, GraphWriteRepo, InsightRepo, LoginAttemptRepo, TokenRepo, UserRepo,
    WorkRepo,
};
use crate::web::ws_broadcaster::WsBroadcaster;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Surreal<Db>>,
    pub user_repo: Arc<dyn UserRepo>,
    pub login_attempt_repo: Arc<dyn LoginAttemptRepo>,
    pub token_repo: Arc<dyn TokenRepo>,
    pub work_repo: Arc<dyn WorkRepo>,
    pub insight_repo: Arc<dyn InsightRepo>,
    pub concept_repo: Arc<dyn ConceptRepo>,
    pub graph_write_repo: Arc<dyn GraphWriteRepo>,
    pub graph_read_repo: Arc<dyn GraphReadRepo>,
    pub message_publisher: Option<Arc<dyn MessagePublisher>>,
    pub open_library_client: Option<Arc<dyn OpenLibraryPort>>,
    pub google_books_client: Option<Arc<dyn GoogleBooksPort>>,
    pub ws_broadcaster: Arc<WsBroadcaster>,
    pub jwt_secret: String,
}
