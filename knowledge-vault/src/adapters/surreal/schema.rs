pub const SCHEMA_SQL: &str = r#"
DEFINE TABLE IF NOT EXISTS users SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS email        ON users TYPE string ASSERT string::is::email($value);
DEFINE FIELD IF NOT EXISTS password_hash ON users TYPE string;
DEFINE FIELD IF NOT EXISTS role         ON users TYPE string DEFAULT 'admin';
DEFINE FIELD IF NOT EXISTS created_at   ON users TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS users_email  ON users COLUMNS email UNIQUE;

DEFINE TABLE IF NOT EXISTS login_attempts SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS email        ON login_attempts TYPE string;
DEFINE FIELD IF NOT EXISTS succeeded    ON login_attempts TYPE bool;
DEFINE FIELD IF NOT EXISTS attempted_at ON login_attempts TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS login_attempts_email_time ON login_attempts COLUMNS email, attempted_at;
"#;

pub async fn run_migrations(db: &surrealdb::Surreal<surrealdb::engine::local::Db>) -> Result<(), surrealdb::Error> {
    db.query(SCHEMA_SQL).await?;
    Ok(())
}
