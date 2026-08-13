CREATE TABLE rate_limit_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scope_type TEXT NOT NULL CHECK (scope_type IN ('global', 'role', 'user')),
    role_id UUID REFERENCES roles(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    endpoint TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- One policy per scope/endpoint combination. endpoint NULL means "generic"
-- (applies to all endpoints); COALESCE makes NULLs unique-aware in the index.
CREATE UNIQUE INDEX idx_rlp_global ON rate_limit_policies(scope_type, COALESCE(endpoint, ''))
    WHERE scope_type = 'global';
CREATE UNIQUE INDEX idx_rlp_role ON rate_limit_policies(scope_type, role_id, COALESCE(endpoint, ''))
    WHERE scope_type = 'role';
CREATE UNIQUE INDEX idx_rlp_user ON rate_limit_policies(scope_type, user_id, COALESCE(endpoint, ''))
    WHERE scope_type = 'user';
