-- V5: server-owned memory service hardening.
--
-- Idempotent writes (content hash), an audit log for significant memory
-- mutations, and chunked long documents.

-- Idempotency: SHA-256 of the trimmed note text. Re-ingesting identical
-- content returns the existing note instead of duplicating it.
--
-- The index is deliberately NON-unique: historical data may contain exact
-- duplicates, and removing them would be a destructive migration. Idempotency
-- is enforced in the application (find-then-insert); the index accelerates
-- that lookup.
ALTER TABLE notes ADD COLUMN content_hash TEXT;
UPDATE notes SET content_hash = encode(sha256(convert_to(btrim(content), 'UTF8')), 'hex')
    WHERE content_hash IS NULL;
CREATE INDEX idx_notes_content_hash ON notes (content_hash) WHERE content_hash IS NOT NULL;

-- Long-document chunking.
ALTER TABLE notes DROP CONSTRAINT notes_kind_check;
ALTER TABLE notes ADD CONSTRAINT notes_kind_check
    CHECK (kind IN ('note', 'fact', 'preference', 'task', 'event', 'memory', 'chunk'));

-- Audit log: every significant memory mutation (actor, action, target,
-- before/after values). Durable and never silent.
CREATE TABLE audit_log (
    id BIGSERIAL PRIMARY KEY,
    actor TEXT NOT NULL DEFAULT 'unknown',
    action TEXT NOT NULL,
    target_type TEXT,
    target_id TEXT,
    old_value JSONB,
    new_value JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_log_created ON audit_log (created_at);
CREATE INDEX idx_audit_log_actor ON audit_log (actor);
CREATE INDEX idx_audit_log_target ON audit_log (target_type, target_id);