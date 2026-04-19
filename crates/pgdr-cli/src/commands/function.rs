use std::collections::BTreeSet;

use clap::Subcommand;
use serde_json::Value;
use tokio_postgres::Client;

use crate::{error::Result, output};

#[derive(Debug, Subcommand)]
pub enum Command {
    List {
        #[arg(long, default_value = "public")]
        schema: String,
    },
    Deps {
        function: String,
        #[arg(long, default_value = "public")]
        schema: String,
    },
}

pub async fn run(cmd: Command, client: &Client) -> Result<()> {
    match cmd {
        Command::List { schema } => list(client, &schema).await,
        Command::Deps { function, schema } => deps(client, &function, &schema).await,
    }
}

async fn list(client: &Client, schema: &str) -> Result<()> {
    let rows = client
        .query(
            "SELECT routine_name AS name, routine_type AS type, \
             data_type AS return_type, external_language AS language \
             FROM information_schema.routines \
             WHERE routine_schema = $1 \
             ORDER BY routine_name",
            &[&schema],
        )
        .await?;
    output::print_json(&output::rows_to_json(&rows));
    Ok(())
}

async fn deps(client: &Client, function: &str, schema: &str) -> Result<()> {
    let row = client
        .query_opt(
            "SELECT p.prosrc, l.lanname, pg_get_functiondef(p.oid) AS def \
             FROM pg_proc p \
             JOIN pg_namespace n ON n.oid = p.pronamespace \
             JOIN pg_language l ON l.oid = p.prolang \
             WHERE p.proname = $1 AND n.nspname = $2 \
             LIMIT 1",
            &[&function, &schema],
        )
        .await?;

    let Some(row) = row else {
        output::print_json(&[]);
        return Ok(());
    };

    let prosrc: &str = row.get("prosrc");
    let lanname: &str = row.get("lanname");
    let funcdef: &str = row.get("def");

    let (table_names, fn_names) = extract_refs(prosrc, lanname, funcdef)?;

    if table_names.is_empty() && fn_names.is_empty() {
        output::print_json(&[]);
        return Ok(());
    }

    let table_names: Vec<&str> = table_names.iter().map(String::as_str).collect();
    let fn_names: Vec<&str> = fn_names.iter().map(String::as_str).collect();

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
               UNION ALL \
               SELECT 'function' AS kind, n.nspname AS schema, p.proname AS name \
               FROM pg_proc p \
               JOIN pg_namespace n ON n.oid = p.pronamespace \
               WHERE p.proname = ANY($2) \
                 AND (p.proname <> $3 OR n.nspname <> $4) \
             ) deps \
             GROUP BY kind, schema, name \
             ORDER BY kind, schema, name",
            &[&table_names, &fn_names, &function, &schema],
        )
        .await?;

    output::print_json(&output::rows_to_json(&rows));
    Ok(())
}

/// Parses the function body and returns (table_names, function_names) referenced.
fn extract_refs(
    prosrc: &str,
    lanname: &str,
    funcdef: &str,
) -> Result<(BTreeSet<String>, BTreeSet<String>)> {
    let mut tables = BTreeSet::new();
    let mut functions = BTreeSet::new();

    match lanname {
        "sql" => {
            if let Ok(result) = pg_query::parse(prosrc) {
                collect_from_parse_result(&result, &mut tables, &mut functions);
            }
        }
        _ => {
            if let Ok(json) = pg_query::parse_plpgsql(funcdef) {
                let mut queries = Vec::new();
                collect_plpgsql_queries(&json, &mut queries);
                for query in &queries {
                    if let Ok(result) = pg_query::parse(query) {
                        collect_from_parse_result(&result, &mut tables, &mut functions);
                    }
                }
            }
        }
    }

    Ok((tables, functions))
}

fn collect_from_parse_result(
    result: &pg_query::ParseResult,
    tables: &mut BTreeSet<String>,
    functions: &mut BTreeSet<String>,
) {
    for (name, _ctx) in &result.tables {
        // Strip schema qualifier — we resolve schema via the DB lookup
        let base = name.rsplit_once('.').map_or(name.as_str(), |(_, n)| n);
        tables.insert(base.to_owned());
    }
    for (name, _ctx) in &result.functions {
        let base = name.rsplit_once('.').map_or(name.as_str(), |(_, n)| n);
        functions.insert(base.to_owned());
    }
}

/// Recursively collects all `query` strings from PL/pgSQL expression nodes.
fn collect_plpgsql_queries(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(q)) = map.get("query") {
                out.push(q.clone());
            }
            for v in map.values() {
                collect_plpgsql_queries(v, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_plpgsql_queries(v, out);
            }
        }
        _ => {}
    }
}
