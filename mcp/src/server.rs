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
    #[schemars(description = "Optional agent identity attributed to this write")]
    pub agent_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddFileArgs {
    #[schemars(description = "Absolute path to the file to store")]
    pub path: String,
    #[schemars(description = "Optional description or context for the file")]
    pub description: Option<String>,
    #[schemars(description = "Optional agent identity attributed to this write")]
    pub agent_id: Option<String>,
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
    #[schemars(description = "Include contradicted claim pairs (flagged)")]
    pub include_contradicted: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetClaimArgs {
    #[schemars(description = "Claim id (UUID)")]
    pub id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListClaimsArgs {
    #[schemars(description = "Entity label to list claims for (subject or object)")]
    pub entity_label: String,
    #[schemars(description = "Filter by status: active | contradicted | superseded | rejected | quarantined")]
    pub status: Option<String>,
    #[schemars(description = "Max claims to return (1-200)")]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReindexArgs {
    #[schemars(description = "Max rows to re-embed per type (1-500)")]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ForgetEntityArgs {
    #[schemars(description = "Entity label (or alias) to delete")]
    pub label: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PruneNotesArgs {
    #[schemars(description = "Delete notes older than this many days")]
    pub days: i64,
    #[schemars(description = "Max notes to delete (1-1000)")]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CorrectClaimArgs {
    #[schemars(description = "Claim id (UUID) to correct")]
    pub id: String,
    #[schemars(description = "Corrected predicate (optional)")]
    pub predicate: Option<String>,
    #[schemars(description = "Corrected object label (optional)")]
    pub object_label: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MemoryStatsArgs {}

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
            self.config.verifier_enabled,
            args.agent_id.as_deref(),
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
                let ingested: IngestResult = ingest::ingest_document_chunks(
                    &self.pool,
                    &self.llm,
                    &self.embedder,
                    document.id,
                    &text,
                    self.config.verifier_enabled,
                    args.agent_id.as_deref(),
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
            args.include_contradicted.unwrap_or(false),
        )
        .await
        .map_err(|e| self.err(&e.to_string()))?;
        Ok(self.result_json(&result))
    }

    /// Fetch one evidence-backed claim with its supporting note and any
    /// contradictions.
    #[tool(description = "Get a claim with its evidence, source note, and contradictions")]
    async fn get_claim(
        &self,
        Parameters(args): Parameters<GetClaimArgs>,
    ) -> Result<String, McpError> {
        let id = parse_id(&args.id)?;
        let claim = crate::claims::get_claim(&self.pool, id)
            .await
            .map_err(|e| self.err(&e.to_string()))?
            .ok_or_else(|| McpError::invalid_params("claim not found", None))?;
        let contradictions = crate::claims::contradictions_for_claim(&self.pool, id)
            .await
            .map_err(|e| self.err(&e.to_string()))?;
        Ok(self.result_json(&json!({
            "claim": claim,
            "contradictions": contradictions,
        })))
    }

    /// List evidence-backed claims touching an entity, optionally filtered by
    /// status.
    #[tool(description = "List claims about an entity (subject or object), by status")]
    async fn list_claims(
        &self,
        Parameters(args): Parameters<ListClaimsArgs>,
    ) -> Result<String, McpError> {
        let label = args.entity_label.trim();
        if label.is_empty() {
            return Err(McpError::invalid_params("entity_label must not be empty", None));
        }
        let entity = store::find_entity_by_normalized(&self.pool, &store::normalize_label(label))
            .await
            .map_err(|e| self.err(&e.to_string()))?;
        let entity = match entity {
            Some(e) => e,
            None => {
                return Ok(format!("Entity \"{label}\" not found."));
            }
        };
        let status = args.status.as_deref().filter(|s| !s.is_empty());
        let claims = crate::claims::claims_for_entity(
            &self.pool,
            entity.id,
            status,
            args.limit.unwrap_or(50),
        )
        .await
        .map_err(|e| self.err(&e.to_string()))?;
        Ok(self.result_json(&claims))
    }

    /// Re-embed rows whose embedding model does not match the current one
    /// (stub/no vectors or a stale model). Safe on-demand reindexing.
    #[tool(description = "Re-embed notes, entities, and claims that lack vectors from the current model")]
    async fn reindex_embeddings(
        &self,
        Parameters(args): Parameters<ReindexArgs>,
    ) -> Result<String, McpError> {
        let result = crate::retrieval::reindex_embeddings(
            &self.pool,
            &self.embedder,
            args.limit.unwrap_or(200),
        )
        .await
        .map_err(|e| self.err(&e.to_string()))?;
        Ok(self.result_json(&result))
    }

    /// Permanently delete an entity and the memory derived from it
    /// (relations, claims, provenance). Audited.
    #[tool(description = "Forget an entity and everything derived from it (audited, hard delete)")]
    async fn forget_entity(
        &self,
        Parameters(args): Parameters<ForgetEntityArgs>,
    ) -> Result<String, McpError> {
        let label = args.label.trim();
        if label.is_empty() {
            return Err(McpError::invalid_params("label must not be empty", None));
        }
        let entity = store::find_entity_by_normalized(&self.pool, &store::normalize_label(label))
            .await
            .map_err(|e| self.err(&e.to_string()))?;
        let entity = match entity {
            Some(e) => e,
            None => {
                let alias = store::find_entities_by_alias(&self.pool, &store::normalize_label(label))
                    .await
                    .map_err(|e| self.err(&e.to_string()))?;
                match alias.first() {
                    Some(e) => e.clone(),
                    None => return Ok(format!("Entity \"{label}\" not found.")),
                }
            }
        };

        let claims = crate::claims::claims_for_entity(&self.pool, entity.id, None, 1000)
            .await
            .map_err(|e| self.err(&e.to_string()))?;
        let deleted = store::delete_entity(&self.pool, entity.id)
            .await
            .map_err(|e| self.err(&e.to_string()))?;
        crate::audit::record_new(
            &self.pool,
            "tool:forget_entity",
            crate::audit::action::ENTITY_DELETED,
            Some("entity"),
            Some(&entity.id.to_string()),
            serde_json::json!({ "label": entity.label, "claims_removed": claims.len() }),
        )
        .await;
        Ok(self.result_json(&json!({
            "deleted": deleted,
            "entity": entity.label,
            "claims_removed": claims.len(),
        })))
    }

    /// Delete notes older than `days` days (audited retention cleanup).
    #[tool(description = "Prune notes older than N days (audited retention cleanup)")]
    async fn prune_notes(
        &self,
        Parameters(args): Parameters<PruneNotesArgs>,
    ) -> Result<String, McpError> {
        if args.days < 1 {
            return Err(McpError::invalid_params("days must be >= 1", None));
        }
        let notes = store::notes_older_than(&self.pool, args.days, args.limit.unwrap_or(200))
            .await
            .map_err(|e| self.err(&e.to_string()))?;
        let mut deleted = 0i64;
        for note in &notes {
            let ok = store::delete_note(&self.pool, note.id)
                .await
                .map_err(|e| self.err(&e.to_string()))?;
            if ok {
                deleted += 1;
                crate::audit::record_new(
                    &self.pool,
                    "tool:prune_notes",
                    crate::audit::action::NOTE_DELETED,
                    Some("note"),
                    Some(&note.id.to_string()),
                    serde_json::json!({ "age_days": args.days }),
                )
                .await;
            }
        }
        Ok(self.result_json(&json!({ "deleted_notes": deleted })))
    }

    /// Correct a memory: the old claim is marked `superseded` and a new
    /// corrected claim is committed. Audited.
    #[tool(description = "Correct a claim: supersede it and record the corrected version")]
    async fn correct_claim(
        &self,
        Parameters(args): Parameters<CorrectClaimArgs>,
    ) -> Result<String, McpError> {
        let id = parse_id(&args.id)?;
        let old = crate::claims::get_claim_row(&self.pool, id)
            .await
            .map_err(|e| self.err(&e.to_string()))?
            .ok_or_else(|| McpError::invalid_params("claim not found", None))?;
        if old.status == "superseded" {
            return Err(McpError::invalid_params("claim is already superseded", None));
        }

        let predicate = args
            .predicate
            .unwrap_or_else(|| old.predicate.clone());
        let (object_id, object_label) = match args.object_label {
            Some(label) if !label.trim().is_empty() => {
                let resolved = crate::entity::resolve(&self.pool, &label, "concept")
                    .await
                    .map_err(|e| self.err(&e.to_string()))?;
                (resolved.entity.id, resolved.entity.label.clone())
            }
            _ => (old.object_id, old.proposed_object_label.clone()),
        };

        let corrected = crate::claims::insert_claim(
            &self.pool,
            &crate::claims::NewClaim {
                note_id: old.note_id,
                subject_id: old.subject_id,
                proposed_subject_label: &old.proposed_subject_label,
                predicate: &predicate,
                object_id,
                proposed_object_label: &object_label,
                modality: &old.modality,
                confidence: old.confidence,
                status: "active",
                evidence_span: old.evidence_span.clone(),
                evidence_offset: old.evidence_offset,
                extraction_version: &old.extraction_version,
                source: &old.source,
                source_document_id: old.source_document_id,
                metadata: serde_json::json!({ "corrected": true, "supersedes": old.id }),
            },
        )
        .await
        .map_err(|e| self.err(&e.to_string()))?;

        crate::claims::supersede_claim(&self.pool, old.id, corrected.id)
            .await
            .map_err(|e| self.err(&e.to_string()))?;
        crate::audit::record_new(
            &self.pool,
            "tool:correct_claim",
            crate::audit::action::CLAIM_SUPERSEDED,
            Some("claim"),
            Some(&old.id.to_string()),
            serde_json::json!({ "superseded_by": corrected.id, "predicate": predicate }),
        )
        .await;

        Ok(self.result_json(&json!({
            "superseded_claim_id": old.id,
            "corrected_claim_id": corrected.id,
        })))
    }

    /// Memory observability: counts, claim statuses, verifier stats, and
    /// embedding-model coverage.
    #[tool(description = "Memory service statistics (counts, statuses, verifier, embedding coverage)")]
    async fn memory_stats(
        &self,
        Parameters(_args): Parameters<MemoryStatsArgs>,
    ) -> Result<String, McpError> {
        let notes = store::count_notes(&self.pool).await;
        let notes = notes.unwrap_or(0);
        let entities = store::count_entities(&self.pool).await.unwrap_or(0);
        let relations = store::count_relations(&self.pool).await.unwrap_or(0);
        let claims_by_status = store::count_claims_by_status(&self.pool)
            .await
            .unwrap_or_default();
        let contradictions = store::count_contradictions(&self.pool).await.unwrap_or(0);
        let verified = store::count_claims_verified(&self.pool).await.unwrap_or(0);
        let model = self.embedder.model_id();
        let (stale_notes, stale_entities, stale_claims) = store::embeddings_coverage(
            &self.pool,
            model,
        )
        .await
        .unwrap_or((0, 0, 0));

        Ok(self.result_json(&json!({
            "counts": { "notes": notes, "entities": entities, "relations": relations },
            "claims": {
                "by_status": claims_by_status.into_iter().collect::<std::collections::HashMap<_,_>>(),
                "contradictions": contradictions,
                "verified": verified,
            },
            "embeddings": {
                "model": model,
                "semantic": self.embedder.is_semantic(),
                "stale": { "notes": stale_notes, "entities": stale_entities, "claims": stale_claims },
            },
        })))
    }
}
