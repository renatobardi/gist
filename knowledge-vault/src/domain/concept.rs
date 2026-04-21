use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    pub id: String,
    /// Normalized lowercase name (used as the unique key).
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptRelation {
    pub from_concept_name: String,
    pub to_concept_name: String,
    pub relation_type: String,
    pub strength: f64,
}

/// A node in the graph API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub domain: String,
}

/// An edge in the graph API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub relation_type: String,
    pub strength: f64,
}

/// Full graph data returned by GET /api/graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// A book (work) that mentions a concept.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptBook {
    pub work_id: String,
    pub title: String,
    pub author: String,
}

/// A related concept entry in the concept detail view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedConceptSummary {
    pub id: String,
    pub display_name: String,
    pub domain: String,
    pub relation_type: String,
    pub strength: f64,
}

/// Full concept detail returned by GET /api/concepts/{id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptDetail {
    pub concept: Concept,
    pub books: Vec<ConceptBook>,
    pub related_concepts: Vec<RelatedConceptSummary>,
}

/// Normalize a concept name to lowercase, trimmed.
pub fn normalize_concept_name(display_name: &str) -> String {
    display_name.trim().to_lowercase()
}
