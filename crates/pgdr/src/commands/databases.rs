use crate::error::Result;
use crate::output;
use clap::Subcommand;
use serde_json::Map;
use serde_json::Value;
use tokio_postgres::Client;

#[derive(Debug, Subcommand)]
pub enum Command {
    List,
    View { database: String },
}

pub async fn run(cmd: Command, client: &Client) -> Result<Value> {
    match cmd {
        Command::List => list(client).await,
        Command::View { database } => view(client, &database).await,
    }
}

async fn list(client: &Client) -> Result<Value> {
    let rows = client
        .query(
            "SELECT datname AS name, pg_encoding_to_char(encoding) AS encoding, \
             datcollate AS collation, datctype AS ctype \
             FROM pg_database \
             WHERE datistemplate = false \
             ORDER BY datname",
            &[],
        )
        .await?;
    Ok(Value::Array(output::rows_to_json(&rows)))
}

async fn view(client: &Client, database: &str) -> Result<Value> {
    let row = client
        .query_opt(
            "SELECT d.datname AS name, \
             o.rolname AS owner, \
             pg_encoding_to_char(d.encoding) AS encoding, \
             d.datcollate AS collation, \
             d.datctype AS ctype, \
             d.datistemplate AS is_template, \
             d.datallowconn AS allow_connections, \
             d.datconnlimit AS connection_limit, \
             t.spcname AS tablespace, \
             CASE WHEN has_database_privilege(d.oid, 'CONNECT') \
                  THEN pg_database_size(d.oid) END AS size_bytes, \
             (SELECT s.setconfig FROM pg_db_role_setting s \
              WHERE s.setdatabase = d.oid AND s.setrole = 0) AS config \
             FROM pg_database d \
             JOIN pg_roles o ON o.oid = d.datdba \
             JOIN pg_tablespace t ON t.oid = d.dattablespace \
             WHERE d.datname = $1",
            &[&database],
        )
        .await?;

    let Some(row) = row else {
        return Ok(Value::Null);
    };

    let mut map = Map::new();
    map.insert("name".into(), Value::String(row.get("name")));
    map.insert("owner".into(), Value::String(row.get("owner")));
    map.insert("encoding".into(), Value::String(row.get("encoding")));
    map.insert("collation".into(), Value::String(row.get("collation")));
    map.insert("ctype".into(), Value::String(row.get("ctype")));
    map.insert("is_template".into(), Value::Bool(row.get("is_template")));
    map.insert(
        "allow_connections".into(),
        Value::Bool(row.get("allow_connections")),
    );
    map.insert(
        "connection_limit".into(),
        Value::from(row.get::<_, i32>("connection_limit")),
    );
    map.insert("tablespace".into(), Value::String(row.get("tablespace")));

    let size_bytes: Option<i64> = row.get("size_bytes");
    map.insert(
        "size_bytes".into(),
        size_bytes.map_or(Value::Null, Value::from),
    );

    let config: Option<Vec<String>> = row.get("config");
    map.insert("config".into(), config_to_json(config));

    Ok(Value::Object(map))
}

fn config_to_json(config: Option<Vec<String>>) -> Value {
    let Some(entries) = config else {
        return Value::Null;
    };
    let mut map = Map::new();
    for entry in entries {
        let (key, value) = entry.split_once('=').unwrap_or((entry.as_str(), ""));
        map.insert(key.to_owned(), Value::String(value.to_owned()));
    }
    Value::Object(map)
}
