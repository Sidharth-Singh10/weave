CREATE TABLE analytics_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    event_type TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT now(),
    request_id UUID,
    endpoint TEXT,
    metadata JSONB
);

CREATE INDEX idx_analytics_user_ts ON analytics_events(user_id, timestamp);
CREATE INDEX idx_analytics_type_ts ON analytics_events(event_type, timestamp);
CREATE INDEX idx_analytics_ts ON analytics_events(timestamp);
