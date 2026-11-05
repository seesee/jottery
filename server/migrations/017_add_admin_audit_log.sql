-- A permanent record of what admins did. The two user columns deliberately have no
-- foreign keys: the record of who deleted an account has to outlive that account.
-- admin_user_id is NULL for actions taken with jottery-admin, which has no account.

CREATE TABLE admin_audit_log (
    id TEXT PRIMARY KEY NOT NULL,
    admin_user_id TEXT,
    action TEXT NOT NULL,
    target_user_id TEXT,
    target_resource_id TEXT,
    metadata_json TEXT,
    created_at TEXT NOT NULL,
    ip_address TEXT,
    user_agent TEXT
);

CREATE INDEX idx_admin_audit_created ON admin_audit_log(created_at);
CREATE INDEX idx_admin_audit_admin ON admin_audit_log(admin_user_id, created_at);
CREATE INDEX idx_admin_audit_target ON admin_audit_log(target_user_id, created_at);
CREATE INDEX idx_admin_audit_action ON admin_audit_log(action, created_at);
