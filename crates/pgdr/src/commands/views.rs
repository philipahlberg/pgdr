use crate::error::Result;
use crate::output;
use clap::Subcommand;
use serde_json::Value;
use tokio_postgres::Client;

#[derive(Debug, Subcommand)]
pub enum Command {
    List {
        #[arg(long, default_value = "public")]
        schema: String,
    },
}

pub async fn run(cmd: Command, client: &Client) -> Result<Value> {
    match cmd {
        Command::List { schema } => list(client, &schema).await,
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
