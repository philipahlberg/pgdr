use crate::error::Result;
use crate::output;
use serde_json::Value;
use tokio_postgres::Client;

pub async fn run(
    client: &Client,
    state: Option<&str>,
    database: Option<&str>,
    exclude_internal: bool,
) -> Result<Value> {
    let rows = client
        .query(
            "SELECT pid, leader_pid, backend_type, \
             datname AS database, usename AS user, application_name, \
             client_addr::text AS client_addr, client_hostname, client_port, \
             backend_start, xact_start, query_start, state_change, \
             state, wait_event_type, wait_event, \
             backend_xid::text AS backend_xid, backend_xmin::text AS backend_xmin, \
             query_id, query \
             FROM pg_stat_activity \
             WHERE ($1::text IS NULL OR state = $1) \
             AND ($2::text IS NULL OR datname = $2) \
             AND (NOT $3::bool OR backend_type IN ('client backend', 'parallel worker')) \
             ORDER BY pid",
            &[&state, &database, &exclude_internal],
        )
        .await?;
    Ok(Value::Array(output::rows_to_json(&rows)))
}
