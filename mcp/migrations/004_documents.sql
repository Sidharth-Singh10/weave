CREATE TABLE documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    filename TEXT NOT NULL,
    mime_type TEXT,
    size_bytes BIGINT NOT NULL DEFAULT 0,
    storage_key TEXT NOT NULL UNIQUE,
    extracted_text TEXT,
    description TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_documents_created ON documents (created_at);
