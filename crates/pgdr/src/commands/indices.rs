use crate::error::Result;
use crate::output;
use clap::Subcommand;
use serde_json::Map;
use serde_json::Value;
use tokio_postgres::Client;

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(visible_alias = "ls")]
    List {
        #[arg(long, default_value = "public")]
        schema: String,
        #[arg(long)]
        table: Option<String>,
    },
    #[command(visible_alias = "i")]
    Inspect {
        index: String,
        #[arg(long, default_value = "public")]
        schema: String,
    },
}

pub async fn run(cmd: Command, client: &Client) -> Result<Value> {
    match cmd {
        Command::List { schema, table } => list(client, &schema, table.as_deref()).await,
        Command::Inspect { index, schema } => inspect(client, &index, &schema).await,
    }
}

async fn list(client: &Client, schema: &str, table: Option<&str>) -> Result<Value> {
    let rows = if let Some(t) = table {
        client
            .query(
                "SELECT i.relname AS name, t.relname AS table, \
                 ix.indisunique AS unique, ix.indisprimary AS primary, \
                 am.amname AS method, \
                 pg_get_indexdef(ix.indexrelid) AS definition \
                 FROM pg_index ix \
                 JOIN pg_class i ON i.oid = ix.indexrelid \
                 JOIN pg_class t ON t.oid = ix.indrelid \
                 JOIN pg_am am ON am.oid = i.relam \
                 JOIN pg_namespace n ON n.oid = t.relnamespace \
                 WHERE n.nspname = $1 AND t.relname = $2 \
                 ORDER BY i.relname",
                &[&schema, &t],
            )
            .await?
    } else {
        client
            .query(
                "SELECT i.relname AS name, t.relname AS table, \
                 ix.indisunique AS unique, ix.indisprimary AS primary, \
                 am.amname AS method, \
                 pg_get_indexdef(ix.indexrelid) AS definition \
                 FROM pg_index ix \
                 JOIN pg_class i ON i.oid = ix.indexrelid \
                 JOIN pg_class t ON t.oid = ix.indrelid \
                 JOIN pg_am am ON am.oid = i.relam \
                 JOIN pg_namespace n ON n.oid = t.relnamespace \
                 WHERE n.nspname = $1 \
                 ORDER BY t.relname, i.relname",
                &[&schema],
            )
            .await?
    };
    Ok(Value::Array(output::rows_to_json(&rows)))
}

async fn inspect(client: &Client, index: &str, schema: &str) -> Result<Value> {
    let row = client
        .query_opt(
            "SELECT i.relname AS name, \
             n.nspname AS schema, \
             t.relname AS table, \
             tn.nspname AS table_schema, \
             o.rolname AS owner, \
             am.amname AS method, \
             ix.indisunique AS unique, \
             ix.indisprimary AS primary, \
             ix.indisexclusion AS exclusion, \
             ix.indisvalid AS valid, \
             ix.indisready AS ready, \
             ix.indislive AS live, \
             ix.indnatts AS num_columns, \
             ix.indnkeyatts AS num_key_columns, \
             pg_get_indexdef(ix.indexrelid) AS definition, \
             pg_get_expr(ix.indpred, ix.indrelid) AS predicate, \
             CASE WHEN has_schema_privilege(n.oid, 'USAGE') \
                  THEN pg_relation_size(i.oid) END AS size_bytes \
             FROM pg_index ix \
             JOIN pg_class i ON i.oid = ix.indexrelid \
             JOIN pg_namespace n ON n.oid = i.relnamespace \
             JOIN pg_class t ON t.oid = ix.indrelid \
             JOIN pg_namespace tn ON tn.oid = t.relnamespace \
             JOIN pg_roles o ON o.oid = i.relowner \
             JOIN pg_am am ON am.oid = i.relam \
             WHERE i.relname = $1 AND n.nspname = $2 \
               AND i.relkind IN ('i', 'I') \
             LIMIT 1",
            &[&index, &schema],
        )
        .await?;

    let Some(row) = row else {
        return Ok(Value::Null);
    };

    let column_rows = client
        .query(
            "SELECT a.attname AS name, \
             format_type(a.atttypid, a.atttypmod) AS type, \
             pg_get_indexdef(ix.indexrelid, k.ord::int, true) AS expression, \
             k.ord <= ix.indnkeyatts AS is_key \
             FROM pg_index ix \
             JOIN LATERAL unnest(ix.indkey) WITH ORDINALITY AS k(attnum, ord) ON true \
             LEFT JOIN pg_attribute a ON a.attrelid = ix.indrelid AND a.attnum = k.attnum \
             JOIN pg_class i ON i.oid = ix.indexrelid \
             JOIN pg_namespace n ON n.oid = i.relnamespace \
             WHERE i.relname = $1 AND n.nspname = $2 \
             ORDER BY k.ord",
            &[&index, &schema],
        )
        .await?;

    let mut map = Map::new();
    map.insert("name".into(), Value::String(row.get("name")));
    map.insert("schema".into(), Value::String(row.get("schema")));
    map.insert("table".into(), Value::String(row.get("table")));
    map.insert(
        "table_schema".into(),
        Value::String(row.get("table_schema")),
    );
    map.insert("owner".into(), Value::String(row.get("owner")));
    map.insert("method".into(), Value::String(row.get("method")));
    map.insert("unique".into(), Value::Bool(row.get("unique")));
    map.insert("primary".into(), Value::Bool(row.get("primary")));
    map.insert("exclusion".into(), Value::Bool(row.get("exclusion")));
    map.insert("valid".into(), Value::Bool(row.get("valid")));
    map.insert("ready".into(), Value::Bool(row.get("ready")));
    map.insert("live".into(), Value::Bool(row.get("live")));
    map.insert(
        "num_columns".into(),
        Value::from(row.get::<_, i16>("num_columns")),
    );
    map.insert(
        "num_key_columns".into(),
        Value::from(row.get::<_, i16>("num_key_columns")),
    );
    map.insert("definition".into(), Value::String(row.get("definition")));

    let predicate: Option<String> = row.get("predicate");
    map.insert(
        "predicate".into(),
        predicate.map_or(Value::Null, Value::String),
    );

    let size_bytes: Option<i64> = row.get("size_bytes");
    map.insert(
        "size_bytes".into(),
        size_bytes.map_or(Value::Null, Value::from),
    );

    map.insert(
        "columns".into(),
        Value::Array(output::rows_to_json(&column_rows)),
    );

    Ok(Value::Object(map))
}
