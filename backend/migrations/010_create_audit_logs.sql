CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    action TEXT NOT NULL,
    target_type TEXT,
    target_id UUID,
    old_value JSONB,
    new_value JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ip_hash TEXT
);

CREATE INDEX idx_audit_actor_created ON audit_logs(actor_user_id, created_at);
CREATE INDEX idx_audit_target_created ON audit_logs(target_id, created_at);
CREATE INDEX idx_audit_created ON audit_logs(created_at);
