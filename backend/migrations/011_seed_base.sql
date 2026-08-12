-- Baseline configuration data (deterministic seed; safe to re-run).
-- Schema evolution lives in 001-010; this file only seeds reference data.

-- ---------------------------------------------------------------------------
-- Roles
-- ---------------------------------------------------------------------------
INSERT INTO roles (name, description) VALUES
    ('admin',      'Full platform administrator: users, roles, policies, analytics, audit'),
    ('member',     'Standard member with full graph capabilities'),
    ('researcher', 'Heavy research user with higher quotas'),
    ('guest',      'Limited guest access')
ON CONFLICT (name) DO NOTHING;

-- ---------------------------------------------------------------------------
-- Permissions
-- ---------------------------------------------------------------------------
INSERT INTO permissions (key, description) VALUES
    ('admin.users.read',       'Read the user directory'),
    ('admin.users.update',     'Modify users (role, status, overrides)'),
    ('admin.roles.read',       'Read roles and their permissions'),
    ('admin.roles.update',     'Create, edit and delete roles'),
    ('admin.policies.read',    'Read rate-limit and quota policies'),
    ('admin.policies.update',  'Modify rate-limit and quota policies'),
    ('admin.analytics.read',   'Read platform analytics'),
    ('admin.audit.read',       'Read the audit log'),
    ('graph.ingest',           'Ingest new notes into the graph'),
    ('graph.organize',         'Run AI-assisted graph organization'),
    ('graph.label_community',  'Request AI community labels'),
    ('graph.search',           'Search the knowledge graph')
ON CONFLICT (key) DO NOTHING;

-- ---------------------------------------------------------------------------
-- Role -> permission mappings
-- ---------------------------------------------------------------------------
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r CROSS JOIN permissions p WHERE r.name = 'admin'
ON CONFLICT (role_id, permission_id) DO NOTHING;

INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r
JOIN permissions p ON p.key IN ('graph.ingest','graph.organize','graph.label_community','graph.search')
WHERE r.name IN ('member', 'researcher')
ON CONFLICT (role_id, permission_id) DO NOTHING;

INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r
JOIN permissions p ON p.key = 'graph.search'
WHERE r.name = 'guest'
ON CONFLICT (role_id, permission_id) DO NOTHING;

-- ---------------------------------------------------------------------------
-- Rate-limit policies (global default + per-role)
-- ---------------------------------------------------------------------------
INSERT INTO rate_limit_policies (scope_type, endpoint) VALUES ('global', NULL)
ON CONFLICT (scope_type, endpoint) WHERE scope_type = 'global' DO NOTHING;

INSERT INTO rate_limit_rules (policy_id, metric, time_window, limit_value)
SELECT p.id, metric, time_window, limit_value FROM rate_limit_policies p
CROSS JOIN (VALUES
    ('requests'::text, 'minute'::text, 30::bigint),
    ('requests', 'hour', 500),
    ('requests', 'day', 2000),
    ('tokens', 'day', 500000),
    ('tokens', 'month', 10000000),
    ('concurrent', NULL, 4)
) AS r(metric, time_window, limit_value)
WHERE p.scope_type = 'global' AND p.endpoint IS NULL
ON CONFLICT (policy_id, metric, time_window) DO NOTHING;

-- Per-role baseline policies (endpoint NULL = applies to all graph endpoints).
INSERT INTO rate_limit_policies (scope_type, role_id, endpoint)
SELECT 'role', r.id, NULL FROM roles r WHERE r.name IN ('guest','member','researcher','admin')
ON CONFLICT (scope_type, role_id, endpoint) WHERE scope_type = 'role' DO NOTHING;

INSERT INTO rate_limit_rules (policy_id, metric, time_window, limit_value)
SELECT p.id, r.metric, r.time_window, r.limit_value
FROM rate_limit_policies p
JOIN roles ro ON ro.id = p.role_id
CROSS JOIN (VALUES
    ('guest', 'requests'::text, 'minute'::text, 10::bigint),
    ('guest', 'tokens', 'day', 100000),
    ('guest', 'concurrent', NULL, 1),

    ('member', 'requests', 'minute', 30),
    ('member', 'requests', 'hour', 500),
    ('member', 'requests', 'day', 2000),
    ('member', 'tokens', 'day', 500000),
    ('member', 'tokens', 'month', 10000000),
    ('member', 'concurrent', NULL, 2),

    ('researcher', 'requests', 'minute', 60),
    ('researcher', 'requests', 'hour', 1000),
    ('researcher', 'requests', 'day', 5000),
    ('researcher', 'tokens', 'day', 2000000),
    ('researcher', 'tokens', 'month', 40000000),
    ('researcher', 'concurrent', NULL, 5),

    ('admin', 'requests', 'minute', 300),
    ('admin', 'requests', 'hour', 5000),
    ('admin', 'requests', 'day', 20000),
    ('admin', 'tokens', 'day', 20000000),
    ('admin', 'tokens', 'month', 200000000),
    ('admin', 'concurrent', NULL, 10)
) AS r(role_name, metric, time_window, limit_value)
WHERE p.scope_type = 'role' AND p.endpoint IS NULL AND ro.name = r.role_name
ON CONFLICT (policy_id, metric, time_window) DO NOTHING;

-- Endpoint-specific policies: different graph operations have different costs.
INSERT INTO rate_limit_policies (scope_type, role_id, endpoint)
SELECT 'role', r.id, e.endpoint FROM roles r
CROSS JOIN (VALUES
    ('graph.ingest'), ('graph.organize'), ('graph.label_community'), ('graph.search')
) AS e(endpoint)
WHERE r.name IN ('guest','member','researcher')
ON CONFLICT (scope_type, role_id, endpoint) WHERE scope_type = 'role' DO NOTHING;

INSERT INTO rate_limit_rules (policy_id, metric, time_window, limit_value)
SELECT p.id, 'requests', 'minute', CASE r.name || ':' || p.endpoint
    WHEN 'member:graph.ingest'            THEN 20
    WHEN 'member:graph.organize'          THEN 10
    WHEN 'member:graph.label_community'   THEN 20
    WHEN 'member:graph.search'            THEN 100
    WHEN 'researcher:graph.ingest'        THEN 40
    WHEN 'researcher:graph.organize'      THEN 20
    WHEN 'researcher:graph.label_community' THEN 40
    WHEN 'researcher:graph.search'        THEN 200
    WHEN 'guest:graph.search'             THEN 30
    ELSE 10
END
FROM rate_limit_policies p
JOIN roles r ON r.id = p.role_id
WHERE p.scope_type = 'role' AND p.endpoint IS NOT NULL
  AND r.name IN ('guest','member','researcher')
ON CONFLICT (policy_id, metric, time_window) DO NOTHING;
