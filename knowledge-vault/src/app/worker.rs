use std::sync::Arc;

use serde::Deserialize;
use tracing::{error, info, warn};

use crate::{
    domain::insight::GeminiResponse,
    ports::{
        external::{GeminiError, GeminiPort, OpenLibraryPort},
        repository::{ConceptEdge, ConceptRepo, InsightRepo, WorkRepo},
    },
};

#[derive(Debug, Deserialize)]
struct DiscoveryRequested {
    work_id: String,
    identifier: String,
}

pub struct WorkerService {
    nats: async_nats::Client,
    open_library: Arc<dyn OpenLibraryPort>,
    gemini: Arc<dyn GeminiPort>,
    work_repo: Arc<dyn WorkRepo>,
    concept_repo: Arc<dyn ConceptRepo>,
    insight_repo: Arc<dyn InsightRepo>,
}

impl WorkerService {
    pub fn new(
        nats: async_nats::Client,
        open_library: Arc<dyn OpenLibraryPort>,
        gemini: Arc<dyn GeminiPort>,
        work_repo: Arc<dyn WorkRepo>,
        concept_repo: Arc<dyn ConceptRepo>,
        insight_repo: Arc<dyn InsightRepo>,
    ) -> Self {
        Self {
            nats,
            open_library,
            gemini,
            work_repo,
            concept_repo,
            insight_repo,
        }
    }

    pub async fn start(self) {
        tokio::spawn(async move {
            if let Err(e) = self.run().await {
                error!(error = %e, "worker exited with error");
            }
        });
    }

    async fn run(&self) -> Result<(), String> {
        let mut subscriber = self
            .nats
            .subscribe("discovery.requested")
            .await
            .map_err(|e| format!("failed to subscribe to discovery.requested: {e}"))?;

        info!("Worker subscribed to discovery.requested");

        while let Some(msg) = futures::StreamExt::next(&mut subscriber).await {
            let payload = match std::str::from_utf8(&msg.payload) {
                Ok(s) => s.to_string(),
                Err(e) => {
                    error!(error = %e, "worker received invalid UTF-8 payload");
                    continue;
                }
            };

            let event: DiscoveryRequested = match serde_json::from_str(&payload) {
                Ok(e) => e,
                Err(e) => {
                    error!(error = %e, payload = %payload, "worker received malformed discovery event");
                    continue;
                }
            };

            let work_id = event.work_id.clone();
            info!(work_id = %work_id, "processing discovery event");

            if let Err(e) = self.process(&event).await {
                error!(work_id = %work_id, error = %e, "work processing failed");
                let _ = self
                    .work_repo
                    .update_status(&work_id, "failed", Some(&e))
                    .await;
            }
        }

        Ok(())
    }

    async fn process(&self, event: &DiscoveryRequested) -> Result<(), String> {
        let work_id = &event.work_id;

        self.work_repo
            .update_status(work_id, "processing", None)
            .await
            .map_err(|e| format!("db update_status failed: {e}"))?;

        // Fetch book metadata from Open Library
        let metadata = match self
            .open_library
            .fetch_metadata(&event.identifier)
            .await
            .map_err(|e| format!("OpenLibrary fetch failed: {e}"))?
        {
            Some(m) => m,
            None => {
                warn!(work_id = %work_id, isbn = %event.identifier, "OpenLibrary returned no metadata, using ISBN as title");
                crate::ports::external::BookMetadata {
                    title: format!("ISBN {}", event.identifier),
                    author: String::new(),
                    description: String::new(),
                    subjects: vec![],
                    open_library_id: None,
                }
            }
        };

        // Persist book metadata on the work record
        self.work_repo
            .update_metadata(
                work_id,
                &metadata.title,
                &metadata.author,
                metadata.open_library_id.as_deref(),
            )
            .await
            .map_err(|e| format!("db update_metadata failed: {e}"))?;

        // Call Gemini for concept extraction
        let gemini_response = self
            .gemini
            .extract_concepts(&metadata)
            .await
            .map_err(|e| match e {
                GeminiError::Transient(msg) => format!("transient Gemini error: {msg}"),
                GeminiError::Permanent(msg) => format!("permanent Gemini error: {msg}"),
            })?;

        let raw_json = serde_json::to_string(&gemini_response)
            .map_err(|e| format!("failed to serialize Gemini response: {e}"))?;

        self.persist_graph(work_id, &gemini_response, &raw_json)
            .await?;

        self.work_repo
            .update_status(work_id, "done", None)
            .await
            .map_err(|e| format!("db update_status done failed: {e}"))?;

        info!(work_id = %work_id, "discovery complete");
        Ok(())
    }

    async fn persist_graph(
        &self,
        work_id: &str,
        gemini_response: &GeminiResponse,
        raw_json: &str,
    ) -> Result<(), String> {
        // Upsert all concept nodes
        let mut concept_ids: Vec<(String, f64)> = Vec::new();
        for gemini_concept in &gemini_response.concepts {
            let concept = self
                .concept_repo
                .upsert(gemini_concept)
                .await
                .map_err(|e| format!("concept upsert failed: {e}"))?;
            concept_ids.push((concept.id, gemini_concept.relevance_weight));
        }

        // Create insight node with interpreta edge
        let insight = self
            .insight_repo
            .create(
                work_id,
                &gemini_response.summary,
                &gemini_response.key_points,
                raw_json,
            )
            .await
            .map_err(|e| format!("insight create failed: {e}"))?;

        // Create menciona edges: insight -> concept
        for (concept_id, relevance_weight) in &concept_ids {
            self.insight_repo
                .create_menciona(&insight.id, concept_id, *relevance_weight)
                .await
                .map_err(|e| format!("menciona edge failed: {e}"))?;
        }

        // Create relacionado_a edges: concept -> concept
        for gemini_concept in &gemini_response.concepts {
            for related in &gemini_concept.related_concepts {
                let edge = ConceptEdge {
                    from_name: gemini_concept.name.clone(),
                    to_name: related.name.clone(),
                    relation_type: related.relation_type.clone(),
                    strength: related.strength,
                };
                self.concept_repo
                    .create_relacionado_a(edge)
                    .await
                    .map_err(|e| format!("relacionado_a edge failed: {e}"))?;
            }
        }

        info!(work_id = %work_id, insight_id = %insight.id, concepts = concept_ids.len(), "graph persisted");
        Ok(())
    }
}
