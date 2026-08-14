-- Vector embeddings for GraphRAG retrieval.
-- Requires the pgvector extension (docker image pgvector/pgvector).

CREATE EXTENSION IF NOT EXISTS vector;

ALTER TABLE notes ADD COLUMN embedding vector(384);
ALTER TABLE entities ADD COLUMN embedding vector(384);
ALTER TABLE documents ADD COLUMN embedding vector(384);

CREATE INDEX idx_notes_embedding ON notes USING hnsw (embedding vector_cosine_ops);
CREATE INDEX idx_entities_embedding ON entities USING hnsw (embedding vector_cosine_ops);
