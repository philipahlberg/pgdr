use crate::error::Result;
use crate::output;
use crate::parse;
use clap::Subcommand;
use serde_json::Value;
use std::collections::BTreeSet;
use tokio_postgres::Client;

#[derive(Debug, Subcommand)]
pub enum Command {
    List {
        #[arg(long, default_value = "public")]
        schema: String,
    },
    View {
        view: String,
        #[arg(long, default_value = "public")]
        schema: String,
    },
}

pub async fn run(cmd: Command, client: &Client) -> Result<Value> {
    match cmd {
        Command::List { schema } => list(client, &schema).await,
        Command::View { view, schema } => view_one(client, &view, &schema).await,
    }
}

async fn list(client: &Client, schema: &str) -> Result<Value> {
    let rows = client
        .query(
            "SELECT table_name AS name, view_definition AS definition \
             FROM information_schema.views \
             WHERE table_schema = $1 \
             ORDER BY table_name",
            &[&schema],
        )
        .await?;
    Ok(Value::Array(output::rows_to_json(&rows)))
}

async fn view_one(client: &Client, view: &str, schema: &str) -> Result<Value> {
    let row = client
        .query_opt(
            "SELECT \
               c.relname AS name, \
               n.nspname AS schema, \
               CASE c.relkind WHEN 'v' THEN 'view' WHEN 'm' THEN 'materialized_view' \
                              ELSE c.relkind::text END AS kind, \
               o.rolname AS owner, \
               pg_get_viewdef(c.oid, true) AS definition, \
               v.is_updatable = 'YES' AS is_updatable, \
               NULLIF(v.check_option, 'NONE') AS check_option \
             FROM pg_class c \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             JOIN pg_roles o ON o.oid = c.relowner \
             LEFT JOIN information_schema.views v \
               ON v.table_schema = n.nspname AND v.table_name = c.relname \
             WHERE c.relname = $1 AND n.nspname = $2 \
               AND c.relkind IN ('v', 'm') \
             LIMIT 1",
            &[&view, &schema],
        )
        .await?;

    let Some(row) = row else {
        return Ok(Value::Null);
    };

    let definition: String = row.get("definition");

    let column_rows = client
        .query(
            "SELECT a.attname AS name, \
             format_type(a.atttypid, a.atttypmod) AS type, \
             NOT a.attnotnull AS nullable \
             FROM pg_attribute a \
             JOIN pg_class c ON c.oid = a.attrelid \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = $1 AND c.relname = $2 \
               AND a.attnum > 0 AND NOT a.attisdropped \
             ORDER BY a.attnum",
            &[&schema, &view],
        )
        .await?;
    let columns = Value::Array(output::rows_to_json(&column_rows));

    let dependencies = resolve_deps(client, view, schema, &definition).await?;

    let mut map = output::rows_to_json(std::slice::from_ref(&row))
        .into_iter()
        .next()
        .and_then(|v| match v {
            Value::Object(m) => Some(m),
            _ => None,
        })
        .expect("row is object");
    map.insert("columns".into(), columns);
    map.insert("dependencies".into(), dependencies);
    Ok(Value::Object(map))
}

async fn resolve_deps(
    client: &Client,
    view: &str,
    schema: &str,
    definition: &str,
) -> Result<Value> {
    let mut tables = BTreeSet::new();
    let mut functions = BTreeSet::new();
    if let Ok(result) = pg_query::parse(definition) {
        parse::collect_from_parse_result(&result, &mut tables, &mut functions);
    }

    if tables.is_empty() && functions.is_empty() {
        return Ok(Value::Array(vec![]));
    }

    let table_names: Vec<&str> = tables.iter().map(String::as_str).collect();
    let fn_names: Vec<&str> = functions.iter().map(String::as_str).collect();

    let rows = client
        .query(
            "SELECT kind, schema, name FROM ( \
               SELECT \
                 CASE c.relkind \
                   WHEN 'r' THEN 'table' WHEN 'v' THEN 'view' \
                   WHEN 'm' THEN 'materialized_view' WHEN 'S' THEN 'sequence' \
                   WHEN 'f' THEN 'foreign_table' ELSE c.relkind::text END AS kind, \
                 n.nspname AS schema, c.relname AS name \
               FROM pg_class c \
               JOIN pg_namespace n ON n.oid = c.relnamespace \
               WHERE c.relname = ANY($1) AND c.relkind IN ('r', 'v', 'm', 'S', 'f') \
                 AND (c.relname <> $3 OR n.nspname <> $4) \
               UNION ALL \
               SELECT 'function' AS kind, n.nspname AS schema, p.proname AS name \
               FROM pg_proc p \
               JOIN pg_namespace n ON n.oid = p.pronamespace \
               WHERE p.proname = ANY($2) \
             ) deps \
             GROUP BY kind, schema, name \
             ORDER BY kind, schema, name",
            &[&table_names, &fn_names, &view, &schema],
        )
        .await?;

    Ok(Value::Array(output::rows_to_json(&rows)))
}
