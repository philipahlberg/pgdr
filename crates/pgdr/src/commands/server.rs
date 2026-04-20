use crate::error::Result;
use crate::output;
use clap::Subcommand;
use serde_json::Value;
use tokio_postgres::Client;

#[derive(Debug, Subcommand)]
pub enum Command {
    Version,
    Settings,
    Extensions,
}

pub async fn run(cmd: Command, client: &Client) -> Result<Value> {
    match cmd {
        Command::Version => version(client).await,
        Command::Settings => settings(client).await,
        Command::Extensions => extensions(client).await,
    }
}

async fn version(client: &Client) -> Result<Value> {
    let rows = client.query("SELECT version() AS version", &[]).await?;
    Ok(Value::Array(output::rows_to_json(&rows)))
}

async fn settings(client: &Client) -> Result<Value> {
    let rows = client
        .query(
            "SELECT name, setting, unit, category, short_desc AS description, \
             source, reset_val AS default \
             FROM pg_settings \
             ORDER BY category, name",
            &[],
        )
        .await?;
    Ok(Value::Array(output::rows_to_json(&rows)))
}

async fn extensions(client: &Client) -> Result<Value> {
    let rows = client
        .query(
            "SELECT name, default_version, installed_version, comment AS description \
             FROM pg_available_extensions \
             ORDER BY name",
            &[],
        )
        .await?;
    Ok(Value::Array(output::rows_to_json(&rows)))
}
