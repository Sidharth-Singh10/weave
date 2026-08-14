CREATE TABLE notes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    content TEXT NOT NULL,
    summary TEXT,
    kind TEXT NOT NULL DEFAULT 'note'
        CHECK (kind IN ('note', 'fact', 'preference', 'task', 'event', 'memory')),
    tags TEXT[] NOT NULL DEFAULT '{}',
    importance REAL NOT NULL DEFAULT 0,
    source TEXT NOT NULL DEFAULT 'user'
        CHECK (source IN ('user', 'file', 'agent', 'import')),
    metadata JSONB NOT NULL DEFAULT '{}',
    search TSVECTOR GENERATED ALWAYS AS (
        setweight(to_tsvector('english', COALESCE(summary, '')), 'A') ||
        setweight(to_tsvector('english', content), 'B')
    ) STORED,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_notes_search ON notes USING GIN (search);
CREATE INDEX idx_notes_tags ON notes USING GIN (tags);
CREATE INDEX idx_notes_created ON notes (created_at);
CREATE INDEX idx_notes_kind ON notes (kind);
