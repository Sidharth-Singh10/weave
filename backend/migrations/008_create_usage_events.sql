CREATE TABLE usage_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    request_id UUID,
    endpoint TEXT NOT NULL,
    provider TEXT,
    model TEXT,
    input_tokens BIGINT,
    output_tokens BIGINT,
    total_tokens BIGINT,
    latency_ms BIGINT,
    status_code INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    metadata JSONB
);

CREATE INDEX idx_usage_user_created ON usage_events(user_id, created_at);
CREATE INDEX idx_usage_endpoint_created ON usage_events(endpoint, created_at);
