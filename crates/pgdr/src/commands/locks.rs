use crate::error::Result;
use crate::output;
use serde_json::Value;
use tokio_postgres::Client;

pub async fn run(
    client: &Client,
    granted: bool,
    blocked: bool,
    schema: Option<&str>,
    relation: Option<&str>,
    exclude_advisory_locks: bool,
) -> Result<Value> {
    let granted_filter: Option<bool> = if granted {
        Some(true)
    } else if blocked {
        Some(false)
    } else {
        None
    };

    let rows = client
        .query(
            "SELECT \
             l.locktype, \
             l.mode, \
             l.granted, \
             l.fastpath, \
             l.waitstart AS wait_start, \
             d.datname AS database, \
             n.nspname AS schema, \
             c.relname AS relation, \
             l.pid, \
             a.state, \
             a.query, \
             to_jsonb(COALESCE(pg_blocking_pids(l.pid), ARRAY[]::integer[])) AS blocked_by \
             FROM pg_locks l \
             LEFT JOIN pg_database d ON d.oid = l.database \
             LEFT JOIN pg_class c ON c.oid = l.relation \
             LEFT JOIN pg_namespace n ON n.oid = c.relnamespace \
             LEFT JOIN pg_stat_activity a ON a.pid = l.pid \
             WHERE ($1::bool IS NULL OR l.granted = $1) \
             AND ($2::text IS NULL OR n.nspname = $2) \
             AND ($3::text IS NULL OR c.relname = $3) \
             AND (NOT $4::bool OR l.locktype <> 'advisory') \
             ORDER BY l.pid, l.locktype",
            &[&granted_filter, &schema, &relation, &exclude_advisory_locks],
        )
        .await?;

    Ok(Value::Array(output::rows_to_json(&rows)))
}
