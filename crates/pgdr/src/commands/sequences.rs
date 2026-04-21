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
    },
    #[command(visible_alias = "i")]
    Inspect {
        sequence: String,
        #[arg(long, default_value = "public")]
        schema: String,
    },
}

pub async fn run(cmd: Command, client: &Client) -> Result<Value> {
    match cmd {
        Command::List { schema } => list(client, &schema).await,
        Command::Inspect { sequence, schema } => inspect(client, &sequence, &schema).await,
    }
}

async fn list(client: &Client, schema: &str) -> Result<Value> {
    let rows = client
        .query(
            "SELECT sequence_name AS name, data_type, start_value, minimum_value, \
             maximum_value, increment, cycle_option \
             FROM information_schema.sequences \
             WHERE sequence_schema = $1 \
             ORDER BY sequence_name",
            &[&schema],
        )
        .await?;
    Ok(Value::Array(output::rows_to_json(&rows)))
}

async fn inspect(client: &Client, sequence: &str, schema: &str) -> Result<Value> {
    let row = client
        .query_opt(
            "SELECT c.relname AS name, \
             n.nspname AS schema, \
             o.rolname AS owner, \
             format_type(s.seqtypid, NULL) AS data_type, \
             s.seqstart AS start_value, \
             s.seqmin AS minimum_value, \
             s.seqmax AS maximum_value, \
             s.seqincrement AS increment, \
             s.seqcycle AS cycle, \
             s.seqcache AS cache, \
             dc.relname AS owned_by_table, \
             dn.nspname AS owned_by_schema, \
             da.attname AS owned_by_column \
             FROM pg_class c \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             JOIN pg_roles o ON o.oid = c.relowner \
             JOIN pg_sequence s ON s.seqrelid = c.oid \
             LEFT JOIN pg_depend d ON d.objid = c.oid \
               AND d.classid = 'pg_class'::regclass \
               AND d.refclassid = 'pg_class'::regclass \
               AND d.deptype = 'a' \
             LEFT JOIN pg_class dc ON dc.oid = d.refobjid \
             LEFT JOIN pg_namespace dn ON dn.oid = dc.relnamespace \
             LEFT JOIN pg_attribute da ON da.attrelid = d.refobjid \
               AND da.attnum = d.refobjsubid \
             WHERE c.relname = $1 AND n.nspname = $2 \
               AND c.relkind = 'S' \
             LIMIT 1",
            &[&sequence, &schema],
        )
        .await?;

    let Some(row) = row else {
        return Ok(Value::Null);
    };

    let mut map = Map::new();
    map.insert("name".into(), Value::String(row.get("name")));
    map.insert("schema".into(), Value::String(row.get("schema")));
    map.insert("owner".into(), Value::String(row.get("owner")));
    map.insert("data_type".into(), Value::String(row.get("data_type")));
    map.insert(
        "start_value".into(),
        Value::from(row.get::<_, i64>("start_value")),
    );
    map.insert(
        "minimum_value".into(),
        Value::from(row.get::<_, i64>("minimum_value")),
    );
    map.insert(
        "maximum_value".into(),
        Value::from(row.get::<_, i64>("maximum_value")),
    );
    map.insert(
        "increment".into(),
        Value::from(row.get::<_, i64>("increment")),
    );
    map.insert("cycle".into(), Value::Bool(row.get("cycle")));
    map.insert("cache".into(), Value::from(row.get::<_, i64>("cache")));

    let owned_by_table: Option<String> = row.get("owned_by_table");
    let owned_by_schema: Option<String> = row.get("owned_by_schema");
    let owned_by_column: Option<String> = row.get("owned_by_column");
    let owned_by = match (owned_by_schema, owned_by_table, owned_by_column) {
        (Some(s), Some(t), Some(c)) => {
            let mut m = Map::new();
            m.insert("schema".into(), Value::String(s));
            m.insert("table".into(), Value::String(t));
            m.insert("column".into(), Value::String(c));
            Value::Object(m)
        }
        _ => Value::Null,
    };
    map.insert("owned_by".into(), owned_by);

    let last_value_row = client
        .query_opt(
            &format!(
                "SELECT last_value, is_called FROM {}.{}",
                quote_ident(schema),
                quote_ident(sequence),
            ),
            &[],
        )
        .await?;
    let (last_value, is_called) = match last_value_row {
        Some(r) => (
            Value::from(r.get::<_, i64>("last_value")),
            Value::Bool(r.get("is_called")),
        ),
        None => (Value::Null, Value::Null),
    };
    map.insert("last_value".into(), last_value);
    map.insert("is_called".into(), is_called);

    Ok(Value::Object(map))
}

fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}
