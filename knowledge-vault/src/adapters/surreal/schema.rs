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

DEFINE TABLE IF NOT EXISTS personal_access_tokens SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS user_id    ON personal_access_tokens TYPE string;
DEFINE FIELD IF NOT EXISTS name       ON personal_access_tokens TYPE string;
DEFINE FIELD IF NOT EXISTS token_hash ON personal_access_tokens TYPE string;
DEFINE FIELD IF NOT EXISTS created_at ON personal_access_tokens TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS revoked_at ON personal_access_tokens TYPE option<datetime>;
DEFINE INDEX IF NOT EXISTS pat_token_hash ON personal_access_tokens COLUMNS token_hash UNIQUE;

DEFINE TABLE IF NOT EXISTS work SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS title          ON work TYPE string DEFAULT '';
DEFINE FIELD IF NOT EXISTS author         ON work TYPE string DEFAULT '';
DEFINE FIELD IF NOT EXISTS isbn           ON work TYPE option<string>;
DEFINE FIELD IF NOT EXISTS open_library_id ON work TYPE option<string>;
DEFINE FIELD IF NOT EXISTS status         ON work TYPE string DEFAULT 'pending';
DEFINE FIELD IF NOT EXISTS error_msg      ON work TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at     ON work TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at     ON work TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS work_isbn ON work COLUMNS isbn UNIQUE;
DEFINE INDEX IF NOT EXISTS work_ol_id ON work COLUMNS open_library_id UNIQUE;
"#;

pub async fn run_migrations(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<(), surrealdb::Error> {
    db.query(SCHEMA_SQL).await?;
    Ok(())
}
