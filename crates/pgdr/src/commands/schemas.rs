use crate::error::Result;
use crate::output;
use clap::Subcommand;
use serde_json::Map;
use serde_json::Value;
use tokio_postgres::Client;

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(visible_alias = "ls")]
    List,
    #[command(visible_alias = "i")]
    Inspect { schema: String },
}

pub async fn run(cmd: Command, client: &Client) -> Result<Value> {
    match cmd {
        Command::List => list(client).await,
        Command::Inspect { schema } => inspect(client, &schema).await,
    }
}

async fn list(client: &Client) -> Result<Value> {
    let rows = client
        .query(
            "SELECT schema_name AS name, schema_owner AS owner \
             FROM information_schema.schemata \
             ORDER BY schema_name",
            &[],
        )
        .await?;
    Ok(Value::Array(output::rows_to_json(&rows)))
}

async fn inspect(client: &Client, schema: &str) -> Result<Value> {
    let row = client
        .query_opt(
            "SELECT n.nspname AS name, \
             o.rolname AS owner, \
             obj_description(n.oid, 'pg_namespace') AS comment, \
             (SELECT count(*) FROM pg_class \
              WHERE relnamespace = n.oid AND relkind = 'r') AS tables, \
             (SELECT count(*) FROM pg_class \
              WHERE relnamespace = n.oid AND relkind = 'v') AS views, \
             (SELECT count(*) FROM pg_class \
              WHERE relnamespace = n.oid AND relkind = 'm') AS materialized_views, \
             (SELECT count(*) FROM pg_class \
              WHERE relnamespace = n.oid AND relkind = 'f') AS foreign_tables, \
             (SELECT count(*) FROM pg_class \
              WHERE relnamespace = n.oid AND relkind = 'S') AS sequences, \
             (SELECT count(*) FROM pg_proc \
              WHERE pronamespace = n.oid) AS functions \
             FROM pg_namespace n \
             JOIN pg_roles o ON o.oid = n.nspowner \
             WHERE n.nspname = $1",
            &[&schema],
        )
        .await?;

    let Some(row) = row else {
        return Ok(Value::Null);
    };

    let mut map = Map::new();
    map.insert("name".into(), Value::String(row.get("name")));
    map.insert("owner".into(), Value::String(row.get("owner")));

    let comment: Option<String> = row.get("comment");
    map.insert("comment".into(), comment.map_or(Value::Null, Value::String));

    let mut objects = Map::new();
    objects.insert("tables".into(), Value::from(row.get::<_, i64>("tables")));
    objects.insert("views".into(), Value::from(row.get::<_, i64>("views")));
    objects.insert(
        "materialized_views".into(),
        Value::from(row.get::<_, i64>("materialized_views")),
    );
    objects.insert(
        "foreign_tables".into(),
        Value::from(row.get::<_, i64>("foreign_tables")),
    );
    objects.insert(
        "sequences".into(),
        Value::from(row.get::<_, i64>("sequences")),
    );
    objects.insert(
        "functions".into(),
        Value::from(row.get::<_, i64>("functions")),
    );
    map.insert("objects".into(), Value::Object(objects));

    Ok(Value::Object(map))
}
