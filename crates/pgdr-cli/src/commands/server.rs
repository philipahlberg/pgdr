use clap::Subcommand;
use tokio_postgres::Client;

use crate::{error::Result, output};

#[derive(Debug, Subcommand)]
pub enum Command {
    Version,
    Settings,
    Extensions,
}

pub async fn run(cmd: Command, client: &Client) -> Result<()> {
    match cmd {
        Command::Version => version(client).await,
        Command::Settings => settings(client).await,
        Command::Extensions => extensions(client).await,
    }
}

async fn version(client: &Client) -> Result<()> {
    let rows = client
        .query("SELECT version() AS version", &[])
        .await?;
    output::print_json(&output::rows_to_json(&rows));
    Ok(())
}

async fn settings(client: &Client) -> Result<()> {
    let rows = client
        .query(
            "SELECT name, setting, unit, category, short_desc AS description, \
             source, reset_val AS default \
             FROM pg_settings \
             ORDER BY category, name",
            &[],
        )
        .await?;
    output::print_json(&output::rows_to_json(&rows));
    Ok(())
}

async fn extensions(client: &Client) -> Result<()> {
    let rows = client
        .query(
            "SELECT name, default_version, installed_version, comment AS description \
             FROM pg_available_extensions \
             ORDER BY name",
            &[],
        )
        .await?;
    output::print_json(&output::rows_to_json(&rows));
    Ok(())
}
