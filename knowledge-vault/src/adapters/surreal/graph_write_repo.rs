use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::concept::normalize_concept_name;
use crate::domain::insight::GeminiResponse;
use crate::ports::repository::{GraphWriteRepo, RepoError};

pub struct SurrealGraphWriteRepo {
    db: surrealdb::Surreal<surrealdb::engine::local::Db>,
}

impl SurrealGraphWriteRepo {
    pub fn new(db: surrealdb::Surreal<surrealdb::engine::local::Db>) -> Self {
        Self { db }
    }
}

/// Builds a complete SurrealQL transaction as a single string.
///
/// All UUIDs are pre-generated in Rust before entering the query string.
/// UUIDs contain only [0-9a-f-] — safe to embed directly in backtick-quoted record IDs.
/// All other user-controlled strings are passed via $-bindings to prevent injection.
fn build_transaction_sql(
    work_id: &str,
    insight_id: &str,
    gemini_response: &GeminiResponse,
    raw_json: &str,
) -> (String, Vec<(String, serde_json::Value)>) {
    let mut sql = String::from("BEGIN TRANSACTION;\n");
    let mut bindings: Vec<(String, serde_json::Value)> = Vec::new();

    // Idempotency check: skip if this work already has an insight
    sql.push_str(&format!(
        "LET $existing_insight = (SELECT out FROM interpreta WHERE in = type::thing('work', '{work_id}') LIMIT 1);\n"
    ));

    // Create insight node only when none exists for this work
    let summary_key = "summary".to_string();
    let key_points_key = "key_points".to_string();
    let raw_json_key = "raw_json".to_string();

    bindings.push((
        summary_key.clone(),
        serde_json::Value::String(gemini_response.summary.clone()),
    ));
    bindings.push((
        key_points_key.clone(),
        serde_json::json!(gemini_response.key_points),
    ));
    bindings.push((
        raw_json_key.clone(),
        serde_json::Value::String(raw_json.to_string()),
    ));

    sql.push_str(&format!(
        "IF array::len($existing_insight) = 0 THEN \
         (CREATE insight:`{insight_id}` SET summary = $summary, key_points = $key_points, raw_gemini_response = $raw_json, created_at = time::now()) \
         END;\n"
    ));

    // Create interpreta edge only when none exists
    sql.push_str(&format!(
        "IF array::len($existing_insight) = 0 THEN \
         (RELATE work:`{work_id}`->interpreta->insight:`{insight_id}`) \
         END;\n"
    ));

    // Upsert concepts and create edges
    for (i, concept) in gemini_response.concepts.iter().enumerate() {
        let name = normalize_concept_name(&concept.display_name);
        let concept_id = Uuid::new_v4().to_string();

        let name_key = format!("cname_{i}");
        let dn_key = format!("cdisplay_{i}");
        let desc_key = format!("cdesc_{i}");
        let dom_key = format!("cdom_{i}");
        let weight_key = format!("cweight_{i}");

        bindings.push((name_key.clone(), serde_json::Value::String(name.clone())));
        bindings.push((
            dn_key.clone(),
            serde_json::Value::String(concept.display_name.clone()),
        ));
        bindings.push((
            desc_key.clone(),
            serde_json::Value::String(concept.description.clone()),
        ));
        bindings.push((
            dom_key.clone(),
            serde_json::Value::String(concept.domain.clone()),
        ));
        bindings.push((
            weight_key.clone(),
            serde_json::json!(concept.relevance_weight),
        ));

        // Upsert concept
        sql.push_str(&format!(
            "LET $c{i}_existing = (SELECT id FROM concept WHERE name = ${name_key} LIMIT 1);\n"
        ));
        sql.push_str(&format!(
            "LET $c{i}_id = IF array::len($c{i}_existing) > 0 THEN $c{i}_existing[0].id \
             ELSE (CREATE concept:`{concept_id}` SET name = ${name_key}, display_name = ${dn_key}, \
             description = ${desc_key}, domain = ${dom_key}, created_at = time::now() RETURN id)[0].id END;\n"
        ));

        // Determine effective insight id: either newly created or the existing one
        sql.push_str(&format!(
            "LET $eff_insight_{i} = IF array::len($existing_insight) = 0 THEN insight:`{insight_id}` \
             ELSE $existing_insight[0].out END;\n"
        ));

        // menciona edge
        sql.push_str(&format!(
            "RELATE $eff_insight_{i}->menciona->$c{i}_id SET relevance_weight = ${weight_key};\n"
        ));

        // relacionado_a edges
        for (j, rel) in concept.related_concepts.iter().enumerate() {
            let related_name = normalize_concept_name(&rel.name);
            let related_id = Uuid::new_v4().to_string();

            let rname_key = format!("rname_{i}_{j}");
            let rdn_key = format!("rdn_{i}_{j}");
            let rtype_key = format!("rtype_{i}_{j}");
            let rstrength_key = format!("rstrength_{i}_{j}");

            bindings.push((
                rname_key.clone(),
                serde_json::Value::String(related_name.clone()),
            ));
            bindings.push((rdn_key.clone(), serde_json::Value::String(rel.name.clone())));
            bindings.push((
                rtype_key.clone(),
                serde_json::Value::String(rel.relation_type.clone()),
            ));
            bindings.push((rstrength_key.clone(), serde_json::json!(rel.strength)));

            sql.push_str(&format!(
                "LET $r{i}_{j}_existing = (SELECT id FROM concept WHERE name = ${rname_key} LIMIT 1);\n"
            ));
            sql.push_str(&format!(
                "LET $r{i}_{j}_id = IF array::len($r{i}_{j}_existing) > 0 THEN $r{i}_{j}_existing[0].id \
                 ELSE (CREATE concept:`{related_id}` SET name = ${rname_key}, display_name = ${rdn_key}, \
                 description = '', domain = '', created_at = time::now() RETURN id)[0].id END;\n"
            ));
            sql.push_str(&format!(
                "RELATE $c{i}_id->relacionado_a->$r{i}_{j}_id SET relation_type = ${rtype_key}, strength = ${rstrength_key};\n"
            ));
        }
    }

    // Update work status to done
    sql.push_str(&format!(
        "UPDATE work:`{work_id}` SET status = 'done', updated_at = time::now();\n"
    ));

    sql.push_str("COMMIT TRANSACTION;\n");

    (sql, bindings)
}

#[async_trait]
impl GraphWriteRepo for SurrealGraphWriteRepo {
    async fn write_graph_transaction(
        &self,
        work_id: &str,
        gemini_response: &GeminiResponse,
    ) -> Result<(), RepoError> {
        Uuid::parse_str(work_id)
            .map_err(|_| RepoError::Internal(format!("invalid work_id format: {work_id}")))?;

        let raw_json = serde_json::to_string(gemini_response)
            .map_err(|e| RepoError::Internal(format!("serialize gemini response: {e}")))?;

        let insight_id = Uuid::new_v4().to_string();

        let (sql, bindings) =
            build_transaction_sql(work_id, &insight_id, gemini_response, &raw_json);

        let mut q = self.db.query(&sql);
        for (key, val) in bindings {
            q = q.bind((key, val));
        }

        q.await
            .map_err(|e| RepoError::Internal(format!("graph transaction: {e}")))?;

        tracing::info!(work_id = %work_id, insight_id = %insight_id, "graph write transaction committed");
        Ok(())
    }
}
