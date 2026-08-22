-- Admin pipeline tracer permission.
--
-- Grants the admin role read access to the /api/admin/pipeline/* endpoints
-- (the ingest pipeline debugger). Newly inserted so it joins the existing
-- admin role without re-running the whole base seed.

INSERT INTO permissions (key, description) VALUES
    ('admin.pipeline.read', 'Run the pipeline tracer and inspect its output')
ON CONFLICT (key) DO NOTHING;

INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r CROSS JOIN permissions p
WHERE r.name = 'admin' AND p.key = 'admin.pipeline.read'
ON CONFLICT (role_id, permission_id) DO NOTHING;