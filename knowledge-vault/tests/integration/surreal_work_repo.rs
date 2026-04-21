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
