use crate::error::Result;
use crate::output;
use clap::Subcommand;
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
        table: String,
        #[arg(long, default_value = "public")]
        schema: String,
    },
    Get {
        table: String,
        #[arg(long, default_value = "public")]
        schema: String,
        #[arg(long)]
        limit: Option<i64>,
    },
}

pub async fn run(cmd: Command, client: &Client) -> Result<Value> {
    match cmd {
        Command::List { schema } => list(client, &schema).await,
        Command::Inspect { table, schema } => inspect(client, &schema, &table).await,
        Command::Get {
            table,
            schema,
            limit,
        } => get(client, &schema, &table, limit).await,
    }
}

async fn list(client: &Client, schema: &str) -> Result<Value> {
    let rows = client
        .query(
            "SELECT table_name AS name, table_type AS type \
             FROM information_schema.tables \
             WHERE table_schema = $1 \
             ORDER BY table_name",
            &[&schema],
        )
        .await?;
    Ok(Value::Array(output::rows_to_json(&rows)))
}

async fn inspect(client: &Client, schema: &str, table: &str) -> Result<Value> {
    let rows = client
        .query(
            "SELECT \
               c.column_name AS name, \
               c.data_type AS type, \
               c.is_nullable = 'YES' AS nullable, \
               c.column_default AS default, \
               c.character_maximum_length AS max_length, \
               c.numeric_precision AS numeric_precision, \
               c.numeric_scale AS numeric_scale \
             FROM information_schema.columns c \
             WHERE c.table_schema = $1 AND c.table_name = $2 \
             ORDER BY c.ordinal_position",
            &[&schema, &table],
        )
        .await?;
    Ok(Value::Array(output::rows_to_json(&rows)))
}

async fn get(client: &Client, schema: &str, table: &str, limit: Option<i64>) -> Result<Value> {
    let qualified = format!("\"{schema}\".\"{table}\"");
    let rows = if let Some(n) = limit {
        client
            .query(&format!("SELECT * FROM {qualified} LIMIT $1"), &[&n])
            .await?
    } else {
        client
            .query(&format!("SELECT * FROM {qualified}"), &[])
            .await?
    };
    Ok(Value::Array(output::rows_to_json(&rows)))
}
