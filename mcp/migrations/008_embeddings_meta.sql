-- V3: embedding metadata + claim embeddings.
--
-- Every stored vector is stamped with the embedding model that produced it so
-- stale vectors (stub noise, older models) can be detected and reindexed
-- safely. Claims (the durable memory unit) now get embeddings so recall can
-- surface matching claims alongside notes and entities.

ALTER TABLE notes ADD COLUMN embedding_model TEXT;
ALTER TABLE entities ADD COLUMN embedding_model TEXT;
ALTER TABLE documents ADD COLUMN embedding_model TEXT;

ALTER TABLE claims ADD COLUMN embedding vector(384);
ALTER TABLE claims ADD COLUMN embedding_model TEXT;

CREATE INDEX idx_claims_embedding ON claims USING hnsw (embedding vector_cosine_ops);

-- Fast lookup of rows that need (re)indexing.
CREATE INDEX idx_notes_embedding_model ON notes (embedding_model) WHERE embedding_model IS NULL;
CREATE INDEX idx_entities_embedding_model ON entities (embedding_model) WHERE embedding_model IS NULL;
CREATE INDEX idx_claims_embedding_model ON claims (embedding_model) WHERE embedding_model IS NULL;