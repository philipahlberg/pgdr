use crate::error::Result;
use crate::output;
use clap::Subcommand;
use tokio_postgres::Client;

#[derive(Debug, Subcommand)]
pub enum Command {
    List,
}

pub async fn run(cmd: Command, client: &Client) -> Result<()> {
    match cmd {
        Command::List => list(client).await,
    }
}

async fn list(client: &Client) -> Result<()> {
    let rows = client
        .query(
            "SELECT schema_name AS name, schema_owner AS owner \
             FROM information_schema.schemata \
             ORDER BY schema_name",
            &[],
        )
        .await?;
    output::print_json(&output::rows_to_json(&rows));
    Ok(())
}
