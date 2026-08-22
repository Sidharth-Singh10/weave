//! SQL queries for the knowledge/memory store.

use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{Document, Entity, Note, Relation, RelationView};

// ---------------------------------------------------------------------------
// Notes
// ---------------------------------------------------------------------------

pub async fn insert_note(
    pool: &PgPool,
    content: &str,
    summary: Option<&str>,
    kind: &str,
    tags: &[String],
    source: &str,
    source_document_id: Option<Uuid>,
) -> Result<Note, sqlx::Error> {
    sqlx::query_as::<_, Note>(
        r#"
        INSERT INTO notes (content, summary, kind, tags, source, source_document_id)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, content, summary, kind, tags, importance, source, metadata, created_at, updated_at
        "#,
    )
    .bind(content)
    .bind(summary)
    .bind(kind)
    .bind(tags)
    .bind(source)
    .bind(source_document_id)
    .fetch_one(pool)
    .await
}

pub async fn get_note(pool: &PgPool, id: Uuid) -> Result<Option<Note>, sqlx::Error> {
    sqlx::query_as::<_, Note>(
        r#"
        SELECT id, content, summary, kind, tags, importance, source, metadata, created_at, updated_at
        FROM notes WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list_notes(
    pool: &PgPool,
    limit: i64,
    tag: Option<&str>,
    kind: Option<&str>,
) -> Result<Vec<Note>, sqlx::Error> {
    let limit = limit.clamp(1, 200);
    sqlx::query_as::<_, Note>(
        r#"
        SELECT id, content, summary, kind, tags, importance, source, metadata, created_at, updated_at
        FROM notes
        WHERE ($1::text IS NULL OR $1 = ANY(tags))
          AND ($2::text IS NULL OR kind = $2)
        ORDER BY created_at DESC
        LIMIT $3
        "#,
    )
    .bind(tag)
    .bind(kind)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn delete_note(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("DELETE FROM notes WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

pub fn normalize_label(label: &str) -> String {
    label.trim().to_lowercase()
}

pub async fn find_entity_by_normalized(
    pool: &PgPool,
    normalized: &str,
) -> Result<Option<Entity>, sqlx::Error> {
    sqlx::query_as::<_, Entity>(
        r#"
        SELECT id, label, normalized_label, kind, aliases, description, created_at
        FROM entities WHERE normalized_label = $1
        "#,
    )
    .bind(normalized)
    .fetch_optional(pool)
    .await
}

// Used by entity resolution in later phases.
#[allow(dead_code)]
pub async fn find_entity_by_alias(
    pool: &PgPool,
    normalized: &str,
) -> Result<Option<Entity>, sqlx::Error> {
    sqlx::query_as::<_, Entity>(
        r#"
        SELECT id, label, normalized_label, kind, aliases, description, created_at
        FROM entities WHERE $1 = ANY(aliases)
        "#,
    )
    .bind(normalized)
    .fetch_optional(pool)
    .await
}

/// All entities whose aliases contain `normalized` (case-insensitive). More
/// than one hit means the alias is ambiguous and should not auto-resolve.
pub async fn find_entities_by_alias(
    pool: &PgPool,
    normalized: &str,
) -> Result<Vec<Entity>, sqlx::Error> {
    sqlx::query_as::<_, Entity>(
        r#"
        SELECT id, label, normalized_label, kind, aliases, description, created_at
        FROM entities
        WHERE EXISTS (SELECT 1 FROM unnest(aliases) a WHERE lower(a) = lower($1))
        ORDER BY created_at
        "#,
    )
    .bind(normalized)
    .fetch_all(pool)
    .await
}

pub async fn insert_entity(pool: &PgPool, label: &str, kind: &str) -> Result<Entity, sqlx::Error> {
    sqlx::query_as::<_, Entity>(
        r#"
        INSERT INTO entities (label, normalized_label, kind)
        VALUES ($1, $2, $3)
        RETURNING id, label, normalized_label, kind, aliases, description, created_at
        "#,
    )
    .bind(label)
    .bind(normalize_label(label))
    .bind(kind)
    .fetch_one(pool)
    .await
}

/// Look up an entity by label (normalized) or create it.
pub async fn get_or_create_entity(
    pool: &PgPool,
    label: &str,
    kind: &str,
) -> Result<Entity, sqlx::Error> {
    let normalized = normalize_label(label);
    if let Some(entity) = find_entity_by_normalized(pool, &normalized).await? {
        return Ok(entity);
    }
    insert_entity(pool, label, kind).await
}

pub async fn count_entities(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM entities")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

// ---------------------------------------------------------------------------
// Relations
// ---------------------------------------------------------------------------

/// Insert a relation if it does not already exist. Returns `None` on a
/// duplicate (unique source/target/relation).
pub async fn insert_relation(
    pool: &PgPool,
    source_id: Uuid,
    target_id: Uuid,
    relation: &str,
) -> Result<Option<Relation>, sqlx::Error> {
    let result = sqlx::query_as::<_, Relation>(
        r#"
        INSERT INTO relations (source_id, target_id, relation)
        VALUES ($1, $2, $3)
        ON CONFLICT (source_id, target_id, relation) DO NOTHING
        RETURNING id, source_id, target_id, relation, weight, created_at
        "#,
    )
    .bind(source_id)
    .bind(target_id)
    .bind(relation)
    .fetch_optional(pool)
    .await?;
    Ok(result)
}

/// Look up an existing relation (used when an insert hit the unique
/// constraint).
pub async fn find_relation(
    pool: &PgPool,
    source_id: Uuid,
    target_id: Uuid,
    relation: &str,
) -> Result<Option<Relation>, sqlx::Error> {
    sqlx::query_as::<_, Relation>(
        r#"
        SELECT id, source_id, target_id, relation, weight, created_at
        FROM relations WHERE source_id = $1 AND target_id = $2 AND relation = $3
        "#,
    )
    .bind(source_id)
    .bind(target_id)
    .bind(relation)
    .fetch_optional(pool)
    .await
}

pub async fn count_relations(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM relations")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

/// Entities whose label or alias appears in `text` (case-insensitive
/// substring). Most-specific labels first, capped at 50 — used as LLM context
/// so extraction reuses existing labels instead of duplicating concepts.
pub async fn find_entities_in_text(pool: &PgPool, text: &str) -> Result<Vec<Entity>, sqlx::Error> {
    sqlx::query_as::<_, Entity>(
        r#"
        SELECT id, label, normalized_label, kind, aliases, description, created_at
        FROM entities
        WHERE strpos(lower($1), lower(label)) > 0
           OR EXISTS (SELECT 1 FROM unnest(aliases) a WHERE strpos(lower($1), lower(a)) > 0)
        ORDER BY length(label) DESC
        LIMIT 50
        "#,
    )
    .bind(text)
    .fetch_all(pool)
    .await
}

// ---------------------------------------------------------------------------
// Provenance
// ---------------------------------------------------------------------------

pub async fn link_note_entity(
    pool: &PgPool,
    note_id: Uuid,
    entity_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO note_entities (note_id, entity_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(note_id)
    .bind(entity_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn link_note_relation(
    pool: &PgPool,
    note_id: Uuid,
    relation_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO note_relations (note_id, relation_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(note_id)
    .bind(relation_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Entities a note references (provenance).
pub async fn entities_for_note(pool: &PgPool, note_id: Uuid) -> Result<Vec<Entity>, sqlx::Error> {
    sqlx::query_as::<_, Entity>(
        r#"
        SELECT e.id, e.label, e.normalized_label, e.kind, e.aliases, e.description, e.created_at
        FROM note_entities ne
        JOIN entities e ON e.id = ne.entity_id
        WHERE ne.note_id = $1
        ORDER BY e.label
        "#,
    )
    .bind(note_id)
    .fetch_all(pool)
    .await
}

/// Relations a note created (provenance), with endpoint labels.
pub async fn relations_for_note(
    pool: &PgPool,
    note_id: Uuid,
) -> Result<Vec<RelationView>, sqlx::Error> {
    sqlx::query_as::<_, RelationView>(
        r#"
        SELECT r.id AS relation_id, r.relation,
               e1.label AS source_label, e2.label AS target_label,
               r.source_id, r.target_id
        FROM note_relations nr
        JOIN relations r ON r.id = nr.relation_id
        JOIN entities e1 ON e1.id = r.source_id
        JOIN entities e2 ON e2.id = r.target_id
        WHERE nr.note_id = $1
        ORDER BY r.relation
        "#,
    )
    .bind(note_id)
    .fetch_all(pool)
    .await
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// Full-text search over notes (summaries weighted higher), newest first on
/// ties.
pub async fn search_notes(
    pool: &PgPool,
    query: &str,
    limit: i64,
) -> Result<Vec<Note>, sqlx::Error> {
    let limit = limit.clamp(1, 100);
    sqlx::query_as::<_, Note>(
        r#"
        SELECT id, content, summary, kind, tags, importance, source, metadata, created_at, updated_at
        FROM notes
        WHERE search @@ websearch_to_tsquery('english', $1)
        ORDER BY ts_rank(search, websearch_to_tsquery('english', $1)) DESC, created_at DESC
        LIMIT $2
        "#,
    )
    .bind(query)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Keyword match on entity labels/aliases/kind.
pub async fn search_entities(
    pool: &PgPool,
    query: &str,
    limit: i64,
) -> Result<Vec<Entity>, sqlx::Error> {
    let limit = limit.clamp(1, 100);
    sqlx::query_as::<_, Entity>(
        r#"
        SELECT id, label, normalized_label, kind, aliases, description, created_at
        FROM entities
        WHERE strpos(lower(label), $1) > 0
           OR strpos(lower(COALESCE(description, '')), $1) > 0
        ORDER BY label
        LIMIT $2
        "#,
    )
    .bind(query.to_lowercase())
    .bind(limit)
    .fetch_all(pool)
    .await
}

// ---------------------------------------------------------------------------
// Graph traversal
// ---------------------------------------------------------------------------

pub async fn get_entity_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Entity>, sqlx::Error> {
    sqlx::query_as::<_, Entity>(
        r#"
        SELECT id, label, normalized_label, kind, aliases, description, created_at
        FROM entities WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// All relations touching an entity (either direction), with endpoint labels.
pub async fn relations_for_entity(
    pool: &PgPool,
    entity_id: Uuid,
) -> Result<Vec<RelationView>, sqlx::Error> {
    sqlx::query_as::<_, RelationView>(
        r#"
        SELECT r.id AS relation_id, r.relation,
               e1.label AS source_label, e2.label AS target_label,
               r.source_id, r.target_id
        FROM relations r
        JOIN entities e1 ON e1.id = r.source_id
        JOIN entities e2 ON e2.id = r.target_id
        WHERE r.source_id = $1 OR r.target_id = $1
        ORDER BY r.relation
        "#,
    )
    .bind(entity_id)
    .fetch_all(pool)
    .await
}

/// Entities by a set of ids, preserving the given order.
pub async fn entities_by_ids(pool: &PgPool, ids: &[Uuid]) -> Result<Vec<Entity>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    sqlx::query_as::<_, Entity>(
        r#"
        SELECT id, label, normalized_label, kind, aliases, description, created_at
        FROM entities WHERE id = ANY($1)
        "#,
    )
    .bind(ids)
    .fetch_all(pool)
    .await
}

/// Notes that reference an entity (provenance), newest first.
pub async fn notes_for_entity(
    pool: &PgPool,
    entity_id: Uuid,
    limit: i64,
) -> Result<Vec<Note>, sqlx::Error> {
    let limit = limit.clamp(1, 50);
    sqlx::query_as::<_, Note>(
        r#"
        SELECT n.id, n.content, n.summary, n.kind, n.tags, n.importance, n.source, n.metadata,
               n.created_at, n.updated_at
        FROM note_entities ne
        JOIN notes n ON n.id = ne.note_id
        WHERE ne.entity_id = $1
        ORDER BY n.created_at DESC
        LIMIT $2
        "#,
    )
    .bind(entity_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

// ---------------------------------------------------------------------------
// Embeddings
// ---------------------------------------------------------------------------

pub async fn set_note_embedding(
    pool: &PgPool,
    id: Uuid,
    embedding: &[f32],
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE notes SET embedding = $1 WHERE id = $2")
        .bind(pgvector::Vector::from(embedding.to_vec()))
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_entity_embedding(
    pool: &PgPool,
    id: Uuid,
    embedding: &[f32],
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE entities SET embedding = $1 WHERE id = $2")
        .bind(pgvector::Vector::from(embedding.to_vec()))
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Notes nearest to `embedding` by cosine distance.
pub async fn vector_search_notes(
    pool: &PgPool,
    embedding: &[f32],
    limit: i64,
) -> Result<Vec<Note>, sqlx::Error> {
    let limit = limit.clamp(1, 100);
    sqlx::query_as::<_, Note>(
        r#"
        SELECT id, content, summary, kind, tags, importance, source, metadata, created_at, updated_at
        FROM notes
        WHERE embedding IS NOT NULL
        ORDER BY embedding <=> $1::vector
        LIMIT $2
        "#,
    )
    .bind(pgvector::Vector::from(embedding.to_vec()))
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Entities nearest to `embedding` by cosine distance.
pub async fn vector_search_entities(
    pool: &PgPool,
    embedding: &[f32],
    limit: i64,
) -> Result<Vec<Entity>, sqlx::Error> {
    let limit = limit.clamp(1, 100);
    sqlx::query_as::<_, Entity>(
        r#"
        SELECT id, label, normalized_label, kind, aliases, description, created_at
        FROM entities
        WHERE embedding IS NOT NULL
        ORDER BY embedding <=> $1::vector
        LIMIT $2
        "#,
    )
    .bind(pgvector::Vector::from(embedding.to_vec()))
    .bind(limit)
    .fetch_all(pool)
    .await
}

// ---------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------

pub async fn insert_document(
    pool: &PgPool,
    filename: &str,
    mime_type: Option<&str>,
    size_bytes: i64,
    storage_key: &str,
    extracted_text: Option<&str>,
    description: Option<&str>,
) -> Result<Document, sqlx::Error> {
    sqlx::query_as::<_, Document>(
        r#"
        INSERT INTO documents (filename, mime_type, size_bytes, storage_key, extracted_text, description)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, filename, mime_type, size_bytes, storage_key, extracted_text, description, created_at
        "#,
    )
    .bind(filename)
    .bind(mime_type)
    .bind(size_bytes)
    .bind(storage_key)
    .bind(extracted_text)
    .bind(description)
    .fetch_one(pool)
    .await
}

// Used by file retrieval in later phases.
#[allow(dead_code)]
pub async fn get_document(pool: &PgPool, id: Uuid) -> Result<Option<Document>, sqlx::Error> {
    sqlx::query_as::<_, Document>(
        r#"
        SELECT id, filename, mime_type, size_bytes, storage_key, extracted_text, description, created_at
        FROM documents WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}
