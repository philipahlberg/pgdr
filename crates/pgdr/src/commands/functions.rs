use crate::error::Result;
use crate::output;
use crate::parse;
use clap::Subcommand;
use serde_json::Value;
use std::collections::BTreeSet;
use tokio_postgres::Client;

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(visible_alias = "ls")]
    List {
        #[arg(long, default_value = "public")]
        schema: String,
    },
    #[command(visible_alias = "i")]
    Inspect {
        function: String,
        #[arg(long, default_value = "public")]
        schema: String,
    },
}

pub async fn run(cmd: Command, client: &Client) -> Result<Value> {
    match cmd {
        Command::List { schema } => list(client, &schema).await,
        Command::Inspect { function, schema } => inspect(client, &function, &schema).await,
    }
}

async fn list(client: &Client, schema: &str) -> Result<Value> {
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
    Ok(Value::Array(output::rows_to_json(&rows)))
}

async fn inspect(client: &Client, function: &str, schema: &str) -> Result<Value> {
    let row = client
        .query_opt(
            "SELECT \
             p.proname AS name, \
             n.nspname AS schema, \
             CASE p.prokind WHEN 'f' THEN 'function' WHEN 'p' THEN 'procedure' \
                            WHEN 'a' THEN 'aggregate' WHEN 'w' THEN 'window' \
                            ELSE p.prokind::text END AS kind, \
             l.lanname AS language, \
             pg_get_function_result(p.oid) AS return_type, \
             pg_get_function_arguments(p.oid) AS arguments, \
             CASE p.provolatile WHEN 'i' THEN 'immutable' WHEN 's' THEN 'stable' \
                                WHEN 'v' THEN 'volatile' END AS volatility, \
             p.proisstrict AS strict, \
             p.prosecdef AS security_definer, \
             CASE p.proparallel WHEN 's' THEN 'safe' WHEN 'r' THEN 'restricted' \
                                WHEN 'u' THEN 'unsafe' END AS parallel, \
             o.rolname AS owner, \
             p.prosrc AS source, \
             CASE WHEN p.prokind IN ('f', 'p') \
                  THEN pg_get_functiondef(p.oid) END AS definition \
             FROM pg_proc p \
             JOIN pg_namespace n ON n.oid = p.pronamespace \
             JOIN pg_language l ON l.oid = p.prolang \
             JOIN pg_roles o ON o.oid = p.proowner \
             WHERE p.proname = $1 AND n.nspname = $2 \
             LIMIT 1",
            &[&function, &schema],
        )
        .await?;

    let Some(row) = row else {
        return Ok(Value::Null);
    };

    let language: String = row.get("language");
    let source: Option<String> = row.get("source");
    let definition: Option<String> = row.get("definition");

    let dependencies = match (language.as_str(), source.as_deref(), definition.as_deref()) {
        ("sql" | "plpgsql", Some(src), Some(def)) => {
            resolve_deps(client, function, schema, &language, src, def).await?
        }
        _ => Value::Null,
    };

    let mut map = output::rows_to_json(std::slice::from_ref(&row))
        .into_iter()
        .next()
        .and_then(|v| match v {
            Value::Object(m) => Some(m),
            _ => None,
        })
        .expect("row is object");
    map.insert("dependencies".into(), dependencies);
    Ok(Value::Object(map))
}

async fn resolve_deps(
    client: &Client,
    function: &str,
    schema: &str,
    language: &str,
    source: &str,
    definition: &str,
) -> Result<Value> {
    let (table_names, fn_names) = extract_refs(source, language, definition)?;
    if table_names.is_empty() && fn_names.is_empty() {
        return Ok(Value::Array(vec![]));
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

    Ok(Value::Array(output::rows_to_json(&rows)))
}

fn extract_refs(
    source: &str,
    language: &str,
    definition: &str,
) -> Result<(BTreeSet<String>, BTreeSet<String>)> {
    let mut tables = BTreeSet::new();
    let mut functions = BTreeSet::new();

    match language {
        "sql" => {
            if let Ok(result) = pg_query::parse(source) {
                parse::collect_from_parse_result(&result, &mut tables, &mut functions);
            }
        }
        _ => {
            if let Ok(json) = pg_query::parse_plpgsql(definition) {
                let mut queries = Vec::new();
                parse::collect_plpgsql_queries(&json, &mut queries);
                for query in &queries {
                    if let Ok(result) = pg_query::parse(query) {
                        parse::collect_from_parse_result(&result, &mut tables, &mut functions);
                    }
                }
            }
        }
    }

    Ok((tables, functions))
}
