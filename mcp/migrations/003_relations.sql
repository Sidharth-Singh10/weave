CREATE TABLE relations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    target_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    relation TEXT NOT NULL,
    weight REAL NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (source_id, target_id, relation)
);

CREATE INDEX idx_relations_source ON relations (source_id);
CREATE INDEX idx_relations_target ON relations (target_id);
