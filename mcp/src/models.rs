//! Row types for the knowledge/memory store.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Note {
    pub id: Uuid,
    pub content: String,
    pub summary: Option<String>,
    pub kind: String,
    pub tags: Vec<String>,
    pub importance: f32,
    pub source: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Entity {
    pub id: Uuid,
    pub label: String,
    pub normalized_label: String,
    pub kind: String,
    pub aliases: Vec<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Relation {
    pub id: Uuid,
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub relation: String,
    pub weight: f32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Document {
    pub id: Uuid,
    pub filename: String,
    pub mime_type: Option<String>,
    pub size_bytes: i64,
    pub storage_key: String,
    pub extracted_text: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// What `ingest_note` produced.
#[derive(Debug, Clone, Serialize)]
pub struct IngestResult {
    pub note_id: Uuid,
    pub summary: Option<String>,
    pub entities_added: Vec<String>,
    pub relations_added: Vec<String>,
    pub total_entities: i64,
    pub total_relations: i64,
}

/// A resolved entity plus its graph neighborhood (for `get_node`).
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct EntityView {
    pub entity: Entity,
    pub relations: Vec<RelationView>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct RelationView {
    pub relation_id: Uuid,
    pub relation: String,
    pub source_label: String,
    pub target_label: String,
    pub source_id: Uuid,
    pub target_id: Uuid,
}
