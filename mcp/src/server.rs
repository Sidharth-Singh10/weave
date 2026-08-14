//! The MCP server: exposes Weave's knowledge/memory store as MCP tools over
//! stdio.

use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars;
use rmcp::{tool, tool_router};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::Config;
use crate::embed::Embedder;
use crate::models::{IngestResult, RelationView};
use crate::{files, ingest, store};
use weave_core::llm::OpenCodeClient;

/// The server state. Clone because rmcp handlers borrow `&self`.
#[derive(Clone)]
pub struct MemoryServer {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub llm: Arc<OpenCodeClient>,
    pub embedder: Arc<dyn Embedder>,
}

impl MemoryServer {
    pub fn new(
        pool: PgPool,
        config: Arc<Config>,
        llm: Arc<OpenCodeClient>,
        embedder: Arc<dyn Embedder>,
    ) -> Self {
        Self {
            pool,
            config,
            llm,
            embedder,
        }
    }

    fn result_json<T: serde::Serialize>(&self, value: &T) -> String {
        serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
    }

    fn err(&self, message: &str) -> McpError {
        McpError::internal_error(message.to_string(), None)
    }
}

// ---------------------------------------------------------------------------
// Tool argument schemas
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RememberArgs {
    #[schemars(description = "The note, fact, or memory to store")]
    pub text: String,
    #[schemars(description = "Kind: note | fact | preference | task | event | memory")]
    pub kind: Option<String>,
    #[schemars(description = "Optional tags")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddFileArgs {
    #[schemars(description = "Absolute path to the file to store")]
    pub path: String,
    #[schemars(description = "Optional description or context for the file")]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListNotesArgs {
    #[schemars(description = "Max notes to return (1-200)")]
    pub limit: Option<i64>,
    #[schemars(description = "Filter by tag")]
    pub tag: Option<String>,
    #[schemars(description = "Filter by kind")]
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetNoteArgs {
    #[schemars(description = "Note id (UUID)")]
    pub id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteNoteArgs {
    #[schemars(description = "Note id (UUID)")]
    pub id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchArgs {
    #[schemars(description = "Search query")]
    pub query: String,
    #[schemars(description = "Max results (1-100)")]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetNodeArgs {
    #[schemars(description = "Entity label to look up")]
    pub label: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetRelatedArgs {
    #[schemars(description = "Seed entity label")]
    pub label: String,
    #[schemars(description = "Hop depth to expand (1-3)")]
    pub depth: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RecallArgs {
    #[schemars(description = "Query to recall memories for")]
    pub query: String,
    #[schemars(description = "Max memories to return (1-20)")]
    pub top_k: Option<usize>,
}

fn parse_id(raw: &str) -> Result<Uuid, McpError> {
    Uuid::parse_str(raw.trim())
        .map_err(|_| McpError::invalid_params("invalid note id (expected UUID)", None))
}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

#[tool_router(server_handler)]
impl MemoryServer {
    /// Store a note/fact/memory. Extracts entities and relations, stores them
    /// in the knowledge graph, and returns what was added plus a compact
    /// summary. Use this for anything you want to remember long-term.
    #[tool(
        description = "Store a note, fact, or memory; extract entities and relations into the knowledge graph"
    )]
    async fn remember(
        &self,
        Parameters(args): Parameters<RememberArgs>,
    ) -> Result<String, McpError> {
        let text = args.text.trim().to_string();
        if text.is_empty() {
            return Err(McpError::invalid_params("text must not be empty", None));
        }
        let kind = args.kind.as_deref().unwrap_or("note");
        let tags = args.tags.unwrap_or_default();

        let result = ingest::ingest_note(
            &self.pool,
            &self.llm,
            &self.embedder,
            &text,
            kind,
            &tags,
            "user",
            None,
        )
        .await
        .map_err(|e| self.err(&e.to_string()))?;
        Ok(self.result_json(&result))
    }

    /// Store a file (text, PDF, image, audio, …) on disk, extract text where
    /// possible, and ingest it into the knowledge graph.
    #[tool(description = "Store a file on disk and ingest its text into the knowledge graph")]
    async fn add_file(
        &self,
        Parameters(args): Parameters<AddFileArgs>,
    ) -> Result<String, McpError> {
        let bytes = std::fs::read(&args.path).map_err(|e| {
            McpError::invalid_params(format!("cannot read {}: {e}", args.path), None)
        })?;

        let filename = std::path::Path::new(&args.path)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".to_string());

        let key = files::store_bytes(&self.config.data_dir, &bytes)
            .map_err(|e| self.err(&e.to_string()))?;
        let mime = files::guess_mime(&filename);
        let text = files::extract_text(&filename, &bytes);

        let document = store::insert_document(
            &self.pool,
            &filename,
            Some(mime),
            bytes.len() as i64,
            &key,
            text.as_deref(),
            args.description.as_deref(),
        )
        .await
        .map_err(|e| self.err(&e.to_string()))?;

        let mut result = json!({
            "document_id": document.id,
            "filename": document.filename,
            "mime_type": mime,
            "size_bytes": document.size_bytes,
            "text_extracted": text.is_some(),
        });

        if let Some(text) = text {
            if !text.trim().is_empty() {
                let ingested: IngestResult = ingest::ingest_document_text(
                    &self.pool,
                    &self.llm,
                    &self.embedder,
                    document.id,
                    &text,
                )
                .await
                .map_err(|e| self.err(&e.to_string()))?;
                result["ingested"] = json!({
                    "note_id": ingested.note_id,
                    "entities_added": ingested.entities_added,
                    "relations_added": ingested.relations_added,
                });
            }
        }

        Ok(self.result_json(&result))
    }

    /// List stored notes (summaries, newest first), optionally filtered by
    /// tag or kind.
    #[tool(description = "List notes (summaries only), newest first")]
    async fn list_notes(
        &self,
        Parameters(args): Parameters<ListNotesArgs>,
    ) -> Result<String, McpError> {
        let notes = store::list_notes(
            &self.pool,
            args.limit.unwrap_or(20),
            args.tag.as_deref(),
            args.kind.as_deref(),
        )
        .await
        .map_err(|e| self.err(&e.to_string()))?;
        let view: Vec<serde_json::Value> = notes
            .iter()
            .map(|n| {
                json!({
                    "id": n.id,
                    "summary": n.summary,
                    "kind": n.kind,
                    "tags": n.tags,
                    "source": n.source,
                    "created_at": n.created_at,
                })
            })
            .collect();
        Ok(self.result_json(&view))
    }

    /// Fetch a note's full content plus the entities and relations it created.
    #[tool(description = "Get a note's full content and its graph linkage")]
    async fn get_note(
        &self,
        Parameters(args): Parameters<GetNoteArgs>,
    ) -> Result<String, McpError> {
        let id = parse_id(&args.id)?;
        let note = store::get_note(&self.pool, id)
            .await
            .map_err(|e| self.err(&e.to_string()))?
            .ok_or_else(|| McpError::invalid_params("note not found", None))?;

        let entities = store::entities_for_note(&self.pool, id)
            .await
            .map_err(|e| self.err(&e.to_string()))?;
        let relations: Vec<RelationView> = store::relations_for_note(&self.pool, id)
            .await
            .map_err(|e| self.err(&e.to_string()))?;

        Ok(self.result_json(&json!({
            "note": note,
            "entities": entities,
            "relations": relations,
        })))
    }

    /// Delete a note (entities/relations it created remain in the graph).
    #[tool(description = "Delete a note by id")]
    async fn delete_note(
        &self,
        Parameters(args): Parameters<DeleteNoteArgs>,
    ) -> Result<String, McpError> {
        let id = parse_id(&args.id)?;
        let deleted = store::delete_note(&self.pool, id)
            .await
            .map_err(|e| self.err(&e.to_string()))?;
        if deleted {
            Ok(format!("Deleted note {id}."))
        } else {
            Ok(format!("Note {id} not found."))
        }
    }

    /// Search notes (full-text) and entities (keyword). Returns matching notes
    /// with their summaries and matching entities.
    #[tool(description = "Search notes and entities by keyword")]
    async fn search(&self, Parameters(args): Parameters<SearchArgs>) -> Result<String, McpError> {
        let query = args.query.trim();
        if query.is_empty() {
            return Err(McpError::invalid_params("query must not be empty", None));
        }
        let notes = store::search_notes(&self.pool, query, args.limit.unwrap_or(10))
            .await
            .map_err(|e| self.err(&e.to_string()))?;
        let entities = store::search_entities(&self.pool, query, args.limit.unwrap_or(10))
            .await
            .map_err(|e| self.err(&e.to_string()))?;

        let note_view: Vec<serde_json::Value> = notes
            .iter()
            .map(|n| {
                json!({
                    "id": n.id,
                    "summary": n.summary,
                    "content": n.content,
                    "kind": n.kind,
                    "tags": n.tags,
                    "created_at": n.created_at,
                })
            })
            .collect();

        Ok(self.result_json(&json!({
            "notes": note_view,
            "entities": entities,
        })))
    }

    /// Resolve an entity by label and return it with its immediate relations.
    #[tool(description = "Get an entity and the relations touching it")]
    async fn get_node(
        &self,
        Parameters(args): Parameters<GetNodeArgs>,
    ) -> Result<String, McpError> {
        let label = args.label.trim();
        if label.is_empty() {
            return Err(McpError::invalid_params("label must not be empty", None));
        }
        match crate::graph::get_node(&self.pool, label)
            .await
            .map_err(|e| self.err(&e.to_string()))?
        {
            Some(view) => Ok(self.result_json(&view)),
            None => Ok(format!("Entity \"{label}\" not found.")),
        }
    }

    /// Expand the subgraph around an entity up to `depth` hops.
    #[tool(description = "Get the subgraph around an entity (BFS expansion)")]
    async fn get_related(
        &self,
        Parameters(args): Parameters<GetRelatedArgs>,
    ) -> Result<String, McpError> {
        let label = args.label.trim();
        if label.is_empty() {
            return Err(McpError::invalid_params("label must not be empty", None));
        }
        match crate::graph::get_related(&self.pool, label, args.depth.unwrap_or(1))
            .await
            .map_err(|e| self.err(&e.to_string()))?
        {
            Some(sub) => Ok(self.result_json(&json!({
                "nodes": sub.entities,
                "edges": sub.edges,
            }))),
            None => Ok(format!("Entity \"{label}\" not found.")),
        }
    }

    /// Hybrid retrieval (vector + full-text + graph): recall the memories most
    /// relevant to a query. Returns a compact context block plus structured
    /// notes/entities/relations — the flagship "recall" tool.
    #[tool(description = "Recall memories relevant to a query (GraphRAG hybrid retrieval)")]
    async fn recall_memory(
        &self,
        Parameters(args): Parameters<RecallArgs>,
    ) -> Result<String, McpError> {
        let query = args.query.trim();
        if query.is_empty() {
            return Err(McpError::invalid_params("query must not be empty", None));
        }
        let result = crate::recall::recall_memory(
            &self.pool,
            &self.embedder,
            query,
            args.top_k.unwrap_or(5),
        )
        .await
        .map_err(|e| self.err(&e.to_string()))?;
        Ok(self.result_json(&result))
    }
}
