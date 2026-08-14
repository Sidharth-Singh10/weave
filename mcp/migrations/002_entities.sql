CREATE TABLE entities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    label TEXT NOT NULL,
    normalized_label TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL DEFAULT 'concept',
    aliases TEXT[] NOT NULL DEFAULT '{}',
    description TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_entities_normalized ON entities (normalized_label);
CREATE INDEX idx_entities_label ON entities (label);
CREATE INDEX idx_entities_aliases ON entities USING GIN (aliases);
CREATE INDEX idx_entities_kind ON entities (kind);
