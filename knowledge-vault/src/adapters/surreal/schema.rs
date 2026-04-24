pub const SCHEMA_SQL: &str = r#"
DEFINE TABLE IF NOT EXISTS users SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS email        ON users TYPE string ASSERT string::is::email($value);
DEFINE FIELD IF NOT EXISTS password_hash ON users TYPE string;
DEFINE FIELD IF NOT EXISTS role         ON users TYPE string DEFAULT 'admin';
DEFINE FIELD IF NOT EXISTS created_at   ON users TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS display_name ON users TYPE option<string>;
DEFINE FIELD IF NOT EXISTS preferences  ON users TYPE option<object>;
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
DEFINE FIELD IF NOT EXISTS reading_status  ON work TYPE option<string>;
DEFINE FIELD IF NOT EXISTS progress_pct    ON work TYPE option<float>;
DEFINE FIELD last_action ON work TYPE option<string>;
DEFINE FIELD IF NOT EXISTS cover_image_url ON work TYPE option<string>;
DEFINE FIELD IF NOT EXISTS page_count      ON work TYPE option<int>;
DEFINE FIELD IF NOT EXISTS publisher       ON work TYPE option<string>;
DEFINE FIELD IF NOT EXISTS average_rating  ON work TYPE option<float>;
DEFINE FIELD IF NOT EXISTS preview_link    ON work TYPE option<string>;
DEFINE INDEX IF NOT EXISTS work_isbn       ON work COLUMNS isbn UNIQUE;
DEFINE INDEX IF NOT EXISTS work_ol_id      ON work COLUMNS open_library_id UNIQUE;
DEFINE INDEX IF NOT EXISTS work_created_at ON work COLUMNS created_at;

DEFINE TABLE IF NOT EXISTS insight SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS summary             ON insight TYPE string;
DEFINE FIELD IF NOT EXISTS key_points          ON insight TYPE array<string>;
DEFINE FIELD IF NOT EXISTS raw_gemini_response ON insight TYPE string;
DEFINE FIELD IF NOT EXISTS created_at          ON insight TYPE datetime DEFAULT time::now();

DEFINE TABLE IF NOT EXISTS concept SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS name         ON concept TYPE string;
DEFINE FIELD IF NOT EXISTS display_name ON concept TYPE string;
DEFINE FIELD IF NOT EXISTS description  ON concept TYPE string;
DEFINE FIELD IF NOT EXISTS domain       ON concept TYPE string;
DEFINE FIELD IF NOT EXISTS created_at   ON concept TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS concept_name ON concept COLUMNS name UNIQUE;

DEFINE TABLE IF NOT EXISTS interpreta SCHEMAFULL TYPE RELATION IN work OUT insight;

DEFINE TABLE IF NOT EXISTS menciona SCHEMAFULL TYPE RELATION IN insight OUT concept;
DEFINE FIELD IF NOT EXISTS relevance_weight ON menciona TYPE float;

DEFINE TABLE IF NOT EXISTS relacionado_a SCHEMAFULL TYPE RELATION IN concept OUT concept;
DEFINE FIELD IF NOT EXISTS relation_type ON relacionado_a TYPE string;
DEFINE FIELD IF NOT EXISTS strength      ON relacionado_a TYPE float;
"#;

pub async fn run_migrations(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<(), surrealdb::Error> {
    db.query(SCHEMA_SQL).await?;
    Ok(())
}
