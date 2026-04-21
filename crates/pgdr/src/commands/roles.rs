use crate::error::Result;
use crate::output;
use clap::Subcommand;
use serde_json::Map;
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use tokio_postgres::Client;

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(visible_alias = "ls")]
    List,
    #[command(visible_alias = "i")]
    Inspect { role: String },
}

pub async fn run(cmd: Command, client: &Client) -> Result<Value> {
    match cmd {
        Command::List => list(client).await,
        Command::Inspect { role } => inspect(client, &role).await,
    }
}

async fn list(client: &Client) -> Result<Value> {
    let rows = client
        .query(
            "SELECT rolname AS name, rolsuper AS superuser, \
             rolcreatedb AS create_db, rolcreaterole AS create_role, \
             rolcanlogin AS can_login, rolreplication AS replication, \
             rolconnlimit AS connection_limit \
             FROM pg_roles \
             ORDER BY rolname",
            &[],
        )
        .await?;
    Ok(Value::Array(output::rows_to_json(&rows)))
}

async fn inspect(client: &Client, role: &str) -> Result<Value> {
    let row = client
        .query_opt(
            "SELECT r.rolname, r.rolsuper, r.rolinherit, r.rolcreaterole, \
             r.rolcreatedb, r.rolcanlogin, r.rolreplication, r.rolbypassrls, \
             r.rolconnlimit, r.rolvaliduntil, r.rolconfig, \
             COALESCE( \
                 (SELECT array_agg(g.rolname ORDER BY g.rolname) \
                  FROM pg_auth_members m \
                  JOIN pg_roles g ON g.oid = m.roleid \
                  WHERE m.member = r.oid), \
                 ARRAY[]::name[] \
             ) AS member_of, \
             COALESCE( \
                 (SELECT array_agg(u.rolname ORDER BY u.rolname) \
                  FROM pg_auth_members m \
                  JOIN pg_roles u ON u.oid = m.member \
                  WHERE m.roleid = r.oid), \
                 ARRAY[]::name[] \
             ) AS members \
             FROM pg_roles r \
             WHERE r.rolname = $1",
            &[&role],
        )
        .await?;

    let Some(row) = row else {
        return Ok(Value::Null);
    };

    let mut map = Map::new();
    map.insert("name".into(), Value::String(row.get("rolname")));
    map.insert("superuser".into(), Value::Bool(row.get("rolsuper")));
    map.insert("inherit".into(), Value::Bool(row.get("rolinherit")));
    map.insert("create_role".into(), Value::Bool(row.get("rolcreaterole")));
    map.insert("create_db".into(), Value::Bool(row.get("rolcreatedb")));
    map.insert("can_login".into(), Value::Bool(row.get("rolcanlogin")));
    map.insert("replication".into(), Value::Bool(row.get("rolreplication")));
    map.insert("bypass_rls".into(), Value::Bool(row.get("rolbypassrls")));
    map.insert(
        "connection_limit".into(),
        Value::from(row.get::<_, i32>("rolconnlimit")),
    );

    let valid_until: Option<time::OffsetDateTime> = row.get("rolvaliduntil");
    map.insert(
        "valid_until".into(),
        valid_until
            .and_then(|v| v.format(&Rfc3339).ok())
            .map_or(Value::Null, Value::String),
    );

    let config: Option<Vec<String>> = row.get("rolconfig");
    map.insert("config".into(), config_to_json(config));

    let member_of: Vec<String> = row.get("member_of");
    map.insert("member_of".into(), to_string_array(member_of));

    let members: Vec<String> = row.get("members");
    map.insert("members".into(), to_string_array(members));

    Ok(Value::Object(map))
}

fn to_string_array(values: Vec<String>) -> Value {
    Value::Array(values.into_iter().map(Value::String).collect())
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
