use clap::Subcommand;
use tokio_postgres::Client;

use crate::{error::Result, output};

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
            "SELECT datname AS name, pg_encoding_to_char(encoding) AS encoding, \
             datcollate AS collation, datctype AS ctype \
             FROM pg_database \
             WHERE datistemplate = false \
             ORDER BY datname",
            &[],
        )
        .await?;
    output::print_json(&output::rows_to_json(&rows));
    Ok(())
}
