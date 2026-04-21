use std::sync::Arc;

use surrealdb::{engine::local::Mem, Surreal};

use knowledge_vault::{
    adapters::surreal::{schema::run_migrations, work_repo::SurrealWorkRepo},
    ports::repository::WorkRepo,
};

async fn make_repo() -> Arc<SurrealWorkRepo> {
    let db: Surreal<surrealdb::engine::local::Db> = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("kv_test").use_db("kv_test").await.unwrap();
    run_migrations(&db).await.unwrap();
    Arc::new(SurrealWorkRepo::new(db))
}

#[tokio::test]
async fn create_work_inserts_record_and_returns_work_id() {
    let repo = make_repo().await;
    let work = repo.create_work("9780132350884").await.unwrap();
    assert!(!work.id.is_empty());
    assert_eq!(work.isbn, Some("9780132350884".to_string()));
    assert_eq!(work.status, "pending");
}

#[tokio::test]
async fn find_by_isbn_returns_some_for_existing_isbn() {
    let repo = make_repo().await;
    let created = repo.create_work("9780132350884").await.unwrap();

    let found = repo.find_by_isbn("9780132350884").await.unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.isbn, Some("9780132350884".to_string()));
    assert_eq!(found.status, "pending");
    assert!(!found.id.is_empty());
    let _ = created; // suppress unused warning
}

#[tokio::test]
async fn find_by_isbn_returns_none_for_unknown_isbn() {
    let repo = make_repo().await;
    let found = repo.find_by_isbn("9780000000000").await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn create_work_by_title_inserts_record_and_returns_work() {
    let repo = make_repo().await;
    let work = repo
        .create_work_by_title("Clean Code", "Robert C. Martin", "/works/OL123W")
        .await
        .unwrap();
    assert!(!work.id.is_empty());
    assert_eq!(work.title, "Clean Code");
    assert_eq!(work.author, "Robert C. Martin");
    assert_eq!(work.open_library_id, Some("/works/OL123W".to_string()));
    assert_eq!(work.isbn, None);
    assert_eq!(work.status, "pending");
}

#[tokio::test]
async fn find_by_open_library_id_returns_some_for_existing_id() {
    let repo = make_repo().await;
    let created = repo
        .create_work_by_title("Clean Code", "Robert C. Martin", "/works/OL123W")
        .await
        .unwrap();

    let found = repo.find_by_open_library_id("/works/OL123W").await.unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.open_library_id, Some("/works/OL123W".to_string()));
    assert_eq!(found.id, created.id);
}

#[tokio::test]
async fn find_by_open_library_id_returns_none_for_unknown_id() {
    let repo = make_repo().await;
    let found = repo
        .find_by_open_library_id("/works/OL999999W")
        .await
        .unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn list_works_returns_all_works() {
    let repo = make_repo().await;
    repo.create_work("9780132350884").await.unwrap();
    repo.create_work("0132350882").await.unwrap();

    let works = repo.list_works(50, 0).await.unwrap();
    assert_eq!(works.len(), 2);
    assert!(works.iter().all(|w| w.status == "pending"));
}

#[tokio::test]
async fn list_works_respects_limit() {
    let repo = make_repo().await;
    repo.create_work("9780132350884").await.unwrap();
    repo.create_work("0132350882").await.unwrap();

    let works = repo.list_works(1, 0).await.unwrap();
    assert_eq!(works.len(), 1);
}

#[tokio::test]
async fn list_works_respects_offset() {
    let repo = make_repo().await;
    repo.create_work("9780132350884").await.unwrap();
    repo.create_work("0132350882").await.unwrap();

    let all = repo.list_works(50, 0).await.unwrap();
    let paginated = repo.list_works(50, 1).await.unwrap();
    assert_eq!(paginated.len(), 1);
    assert_eq!(paginated[0].id, all[1].id);
}

#[tokio::test]
async fn get_work_by_id_returns_some_for_known_id() {
    let repo = make_repo().await;
    let created = repo.create_work("9780132350884").await.unwrap();

    let found = repo.get_work_by_id(&created.id).await.unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.id, created.id);
    assert_eq!(found.isbn, Some("9780132350884".to_string()));
    assert_eq!(found.status, "pending");
}

#[tokio::test]
async fn get_work_by_id_returns_none_for_unknown_id() {
    let repo = make_repo().await;
    let found = repo
        .get_work_by_id("00000000-0000-0000-0000-000000000000")
        .await
        .unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn find_by_id_returns_some_for_existing_work() {
    let repo = make_repo().await;
    let created = repo.create_work("9780132350884").await.unwrap();

    let found = repo.find_by_id(&created.id).await.unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.id, created.id);
    assert_eq!(found.isbn, Some("9780132350884".to_string()));
    assert_eq!(found.status, "pending");
}

#[tokio::test]
async fn find_by_id_returns_none_for_unknown_id() {
    let repo = make_repo().await;
    let found = repo
        .find_by_id("00000000-0000-0000-0000-000000000000")
        .await
        .unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn update_work_status_changes_status() {
    let repo = make_repo().await;
    let created = repo.create_work("9780132350884").await.unwrap();
    assert_eq!(created.status, "pending");

    repo.update_work_status(&created.id, "processing", None)
        .await
        .unwrap();

    let updated = repo.get_work_by_id(&created.id).await.unwrap().unwrap();
    assert_eq!(updated.status, "processing");
    assert!(updated.error_msg.is_none());
}

#[tokio::test]
async fn update_work_status_sets_error_msg_on_failure() {
    let repo = make_repo().await;
    let created = repo.create_work("9780132350884").await.unwrap();

    repo.update_work_status(&created.id, "failed", Some("timeout calling Gemini API"))
        .await
        .unwrap();

    let updated = repo.get_work_by_id(&created.id).await.unwrap().unwrap();
    assert_eq!(updated.status, "failed");
    assert_eq!(
        updated.error_msg.as_deref(),
        Some("timeout calling Gemini API")
    );
}

#[tokio::test]
async fn update_work_status_returns_not_found_for_unknown_id() {
    let repo = make_repo().await;
    let result = repo
        .update_work_status("00000000-0000-0000-0000-000000000000", "processing", None)
        .await;
    assert!(matches!(
        result,
        Err(knowledge_vault::ports::repository::RepoError::NotFound)
    ));
}

#[tokio::test]
async fn reset_to_pending_updates_work_status() {
    let repo = make_repo().await;
    let created = repo.create_work("9780132350884").await.unwrap();

    repo.update_work_status(&created.id, "failed", Some("network error"))
        .await
        .unwrap();

    let reset = repo.reset_to_pending(&created.id).await.unwrap();
    assert_eq!(reset.id, created.id);
    assert_eq!(reset.status, "pending");
}

#[tokio::test]
async fn reset_to_pending_returns_not_found_for_non_failed_work() {
    let repo = make_repo().await;
    let created = repo.create_work("9780132350884").await.unwrap();

    let result = repo.reset_to_pending(&created.id).await;
    assert!(matches!(
        result,
        Err(knowledge_vault::ports::repository::RepoError::NotFound)
    ));
}
