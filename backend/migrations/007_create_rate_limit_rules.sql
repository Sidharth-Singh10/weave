CREATE TABLE rate_limit_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    policy_id UUID NOT NULL REFERENCES rate_limit_policies(id) ON DELETE CASCADE,
    metric TEXT NOT NULL CHECK (metric IN ('requests', 'tokens', 'concurrent')),
    -- NULL time_window is valid for the `concurrent` metric (a live counter).
    time_window TEXT CHECK (time_window IN ('minute', 'hour', 'day', 'month')),
    limit_value BIGINT NOT NULL CHECK (limit_value >= 0),
    UNIQUE (policy_id, metric, time_window)
);

CREATE INDEX idx_rlr_policy ON rate_limit_rules(policy_id);
