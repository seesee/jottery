//! A persistent record of admin actions, for accountability and for working out
//! afterwards what happened to an account.
//!
//! Recording is advisory: it happens after the action, and a failure is logged as
//! an error rather than undoing what the admin did.

use serde::Serialize;
use serde_json::Value;
use sqlx::SqlitePool;
use uuid::Uuid;

pub const USER_APPROVE: &str = "user.approve";
pub const USER_DEACTIVATE: &str = "user.deactivate";
pub const USER_ACTIVATE: &str = "user.activate";
pub const USER_DELETE: &str = "user.delete";
pub const USER_PASSWORD_RESET: &str = "user.password_reset";
pub const USER_ADMIN_GRANT: &str = "user.admin_grant";
pub const USER_ADMIN_REVOKE: &str = "user.admin_revoke";
pub const USER_SETTINGS_UPDATE: &str = "user.settings_update";
pub const DEVICE_REVOKE: &str = "device.revoke";
pub const DEVICE_DELETE: &str = "device.delete";
pub const DEVICE_RENAME: &str = "device.rename";
pub const ADMIN_PASSWORD_CHANGE: &str = "admin.password_change";

/// Who did something, and from where.
#[derive(Clone, Debug, Default)]
pub struct AuditActor {
    /// `None` for jottery-admin, which is run by whoever holds the server.
    pub admin_user_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// What was done, and to what.
#[derive(Debug, Default)]
pub struct AuditEvent<'a> {
    pub action: &'a str,
    pub target_user_id: Option<&'a str>,
    pub target_resource_id: Option<&'a str>,
    /// Never credentials, and nothing a client encrypted.
    pub metadata: Option<Value>,
}

impl<'a> AuditEvent<'a> {
    pub fn on_user(action: &'a str, user_id: &'a str) -> Self {
        Self {
            action,
            target_user_id: Some(user_id),
            ..Self::default()
        }
    }

    pub fn on_device(action: &'a str, device_id: &'a str) -> Self {
        Self {
            action,
            target_resource_id: Some(device_id),
            ..Self::default()
        }
    }

    pub fn with_metadata(self, metadata: Value) -> Self {
        Self {
            metadata: Some(metadata),
            ..self
        }
    }
}

/// Record an action that has happened.
pub async fn record(pool: &SqlitePool, actor: &AuditActor, event: AuditEvent<'_>) {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let metadata = event.metadata.as_ref().map(Value::to_string);

    let result = sqlx::query!(
        "INSERT INTO admin_audit_log
           (id, admin_user_id, action, target_user_id, target_resource_id, metadata_json, created_at, ip_address, user_agent)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        actor.admin_user_id,
        event.action,
        event.target_user_id,
        event.target_resource_id,
        metadata,
        now,
        actor.ip_address,
        actor.user_agent,
    )
    .execute(pool)
    .await;

    if let Err(e) = result {
        tracing::error!(
            "Audit log write failed: action={} admin={:?} target={:?} resource={:?}: {}",
            event.action,
            actor.admin_user_id,
            event.target_user_id,
            event.target_resource_id,
            e
        );
    }
}

/// Narrows a listing of the log. Times are RFC 3339 and inclusive.
#[derive(Debug, Default)]
pub struct AuditFilter {
    pub admin_user_id: Option<String>,
    pub target_user_id: Option<String>,
    pub action: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub id: String,
    pub admin_user_id: Option<String>,
    pub action: String,
    pub target_user_id: Option<String>,
    pub target_resource_id: Option<String>,
    #[serde(skip)]
    pub metadata_json: Option<String>,
    pub created_at: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

fn push_filters<'a>(
    query: &mut sqlx::QueryBuilder<'a, sqlx::Sqlite>,
    filter: &'a AuditFilter,
) {
    query.push(" WHERE 1 = 1");
    if let Some(admin) = &filter.admin_user_id {
        query.push(" AND admin_user_id = ").push_bind(admin);
    }
    if let Some(target) = &filter.target_user_id {
        query.push(" AND target_user_id = ").push_bind(target);
    }
    if let Some(action) = &filter.action {
        query.push(" AND action = ").push_bind(action);
    }
    if let Some(from) = &filter.from {
        query.push(" AND created_at >= ").push_bind(from);
    }
    if let Some(to) = &filter.to {
        query.push(" AND created_at <= ").push_bind(to);
    }
}

/// The newest entries matching `filter`, and how many match in all.
pub async fn list(
    pool: &SqlitePool,
    filter: &AuditFilter,
    limit: i64,
    offset: i64,
) -> Result<(Vec<AuditEntry>, i64), sqlx::Error> {
    let mut query = sqlx::QueryBuilder::new(
        "SELECT id, admin_user_id, action, target_user_id, target_resource_id, metadata_json, created_at, ip_address, user_agent
         FROM admin_audit_log",
    );
    push_filters(&mut query, filter);
    query
        .push(" ORDER BY created_at DESC, id LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);
    let entries = query.build_query_as().fetch_all(pool).await?;

    let mut count = sqlx::QueryBuilder::new("SELECT COUNT(*) FROM admin_audit_log");
    push_filters(&mut count, filter);
    let (total,): (i64,) = count.build_query_as().fetch_one(pool).await?;

    Ok((entries, total))
}
