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
        constraint: String,
        #[arg(long, default_value = "public")]
        schema: String,
        #[arg(long)]
        table: Option<String>,
    },
}

pub async fn run(cmd: Command, client: &Client) -> Result<Value> {
    match cmd {
        Command::List { schema, table } => list(client, &schema, table.as_deref()).await,
        Command::Inspect {
            constraint,
            schema,
            table,
        } => inspect(client, &constraint, &schema, table.as_deref()).await,
    }
}

async fn list(client: &Client, schema: &str, table: Option<&str>) -> Result<Value> {
    let rows = if let Some(t) = table {
        client
            .query(
                "SELECT tc.constraint_name AS name, tc.table_name AS table, \
                 tc.constraint_type AS type, \
                 rc.update_rule, rc.delete_rule, \
                 ccu.table_name AS foreign_table, \
                 ccu.column_name AS foreign_column \
                 FROM information_schema.table_constraints tc \
                 LEFT JOIN information_schema.referential_constraints rc \
                   ON rc.constraint_name = tc.constraint_name \
                   AND rc.constraint_schema = tc.constraint_schema \
                 LEFT JOIN information_schema.constraint_column_usage ccu \
                   ON ccu.constraint_name = tc.constraint_name \
                   AND ccu.constraint_schema = tc.constraint_schema \
                 WHERE tc.constraint_schema = $1 AND tc.table_name = $2 \
                 ORDER BY tc.constraint_name",
                &[&schema, &t],
            )
            .await?
    } else {
        client
            .query(
                "SELECT tc.constraint_name AS name, tc.table_name AS table, \
                 tc.constraint_type AS type, \
                 rc.update_rule, rc.delete_rule, \
                 ccu.table_name AS foreign_table, \
                 ccu.column_name AS foreign_column \
                 FROM information_schema.table_constraints tc \
                 LEFT JOIN information_schema.referential_constraints rc \
                   ON rc.constraint_name = tc.constraint_name \
                   AND rc.constraint_schema = tc.constraint_schema \
                 LEFT JOIN information_schema.constraint_column_usage ccu \
                   ON ccu.constraint_name = tc.constraint_name \
                   AND ccu.constraint_schema = tc.constraint_schema \
                 WHERE tc.constraint_schema = $1 \
                 ORDER BY tc.table_name, tc.constraint_name",
                &[&schema],
            )
            .await?
    };
    Ok(Value::Array(output::rows_to_json(&rows)))
}

async fn inspect(
    client: &Client,
    constraint: &str,
    schema: &str,
    table: Option<&str>,
) -> Result<Value> {
    let row = client
        .query_opt(
            "SELECT c.conname AS name, \
             n.nspname AS schema, \
             t.relname AS table, \
             tn.nspname AS table_schema, \
             CASE c.contype \
               WHEN 'c' THEN 'CHECK' WHEN 'f' THEN 'FOREIGN KEY' \
               WHEN 'p' THEN 'PRIMARY KEY' WHEN 'u' THEN 'UNIQUE' \
               WHEN 'x' THEN 'EXCLUSION' WHEN 't' THEN 'TRIGGER' \
               WHEN 'n' THEN 'NOT NULL' ELSE c.contype::text END AS type, \
             pg_get_constraintdef(c.oid, true) AS definition, \
             c.condeferrable AS deferrable, \
             c.condeferred AS deferred, \
             c.convalidated AS validated, \
             ft.relname AS foreign_table, \
             ftn.nspname AS foreign_schema, \
             CASE c.confupdtype WHEN 'a' THEN 'NO ACTION' WHEN 'r' THEN 'RESTRICT' \
                                WHEN 'c' THEN 'CASCADE' WHEN 'n' THEN 'SET NULL' \
                                WHEN 'd' THEN 'SET DEFAULT' END AS update_rule, \
             CASE c.confdeltype WHEN 'a' THEN 'NO ACTION' WHEN 'r' THEN 'RESTRICT' \
                                WHEN 'c' THEN 'CASCADE' WHEN 'n' THEN 'SET NULL' \
                                WHEN 'd' THEN 'SET DEFAULT' END AS delete_rule, \
             CASE c.confmatchtype WHEN 'f' THEN 'FULL' WHEN 'p' THEN 'PARTIAL' \
                                  WHEN 's' THEN 'SIMPLE' END AS match_type, \
             CASE WHEN c.contype = 'c' \
                  THEN pg_get_expr(c.conbin, c.conrelid, true) END AS check_clause, \
             ARRAY( \
               SELECT a.attname FROM unnest(c.conkey) WITH ORDINALITY AS k(attnum, ord) \
               LEFT JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = k.attnum \
               ORDER BY k.ord \
             ) AS columns, \
             ARRAY( \
               SELECT a.attname FROM unnest(c.confkey) WITH ORDINALITY AS k(attnum, ord) \
               LEFT JOIN pg_attribute a ON a.attrelid = c.confrelid AND a.attnum = k.attnum \
               ORDER BY k.ord \
             ) AS foreign_columns \
             FROM pg_constraint c \
             JOIN pg_namespace n ON n.oid = c.connamespace \
             JOIN pg_class t ON t.oid = c.conrelid \
             JOIN pg_namespace tn ON tn.oid = t.relnamespace \
             LEFT JOIN pg_class ft ON ft.oid = c.confrelid \
             LEFT JOIN pg_namespace ftn ON ftn.oid = ft.relnamespace \
             WHERE c.conname = $1 AND n.nspname = $2 \
               AND ($3::text IS NULL OR t.relname = $3) \
             ORDER BY t.relname \
             LIMIT 1",
            &[&constraint, &schema, &table],
        )
        .await?;

    let Some(row) = row else {
        return Ok(Value::Null);
    };

    let mut map = Map::new();
    map.insert("name".into(), Value::String(row.get("name")));
    map.insert("schema".into(), Value::String(row.get("schema")));
    map.insert("table".into(), Value::String(row.get("table")));
    map.insert(
        "table_schema".into(),
        Value::String(row.get("table_schema")),
    );
    map.insert("type".into(), Value::String(row.get("type")));
    map.insert("definition".into(), Value::String(row.get("definition")));
    map.insert("deferrable".into(), Value::Bool(row.get("deferrable")));
    map.insert("deferred".into(), Value::Bool(row.get("deferred")));
    map.insert("validated".into(), Value::Bool(row.get("validated")));

    let columns: Vec<String> = row.get("columns");
    map.insert("columns".into(), to_string_array(columns));

    let foreign_table: Option<String> = row.get("foreign_table");
    let foreign_schema: Option<String> = row.get("foreign_schema");
    let foreign_columns: Vec<String> = row.get("foreign_columns");
    map.insert(
        "foreign_schema".into(),
        foreign_schema.map_or(Value::Null, Value::String),
    );
    map.insert(
        "foreign_table".into(),
        foreign_table.map_or(Value::Null, Value::String),
    );
    map.insert(
        "foreign_columns".into(),
        if foreign_columns.is_empty() {
            Value::Null
        } else {
            to_string_array(foreign_columns)
        },
    );

    let update_rule: Option<String> = row.get("update_rule");
    let delete_rule: Option<String> = row.get("delete_rule");
    let match_type: Option<String> = row.get("match_type");
    map.insert(
        "update_rule".into(),
        update_rule.map_or(Value::Null, Value::String),
    );
    map.insert(
        "delete_rule".into(),
        delete_rule.map_or(Value::Null, Value::String),
    );
    map.insert(
        "match_type".into(),
        match_type.map_or(Value::Null, Value::String),
    );

    let check_clause: Option<String> = row.get("check_clause");
    map.insert(
        "check_clause".into(),
        check_clause.map_or(Value::Null, Value::String),
    );

    Ok(Value::Object(map))
}

fn to_string_array(values: Vec<String>) -> Value {
    Value::Array(values.into_iter().map(Value::String).collect())
}
