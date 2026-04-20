use crate::error::Result;
use crate::output;
use clap::Subcommand;
use serde_json::Value;
use tokio_postgres::Client;

#[derive(Debug, Subcommand)]
pub enum Command {
    List,
}

pub async fn run(cmd: Command, client: &Client) -> Result<Value> {
    match cmd {
        Command::List => list(client).await,
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
