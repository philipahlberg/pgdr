use clap::Subcommand;
use tokio_postgres::Client;

use crate::{error::Result, output};

#[derive(Debug, Subcommand)]
pub enum Command {
    List {
        #[arg(long, default_value = "public")]
        schema: String,
    },
}

pub async fn run(cmd: Command, client: &Client) -> Result<()> {
    match cmd {
        Command::List { schema } => list(client, &schema).await,
    }
}

async fn list(client: &Client, schema: &str) -> Result<()> {
    let rows = client
        .query(
            "SELECT routine_name AS name, routine_type AS type, \
             data_type AS return_type, external_language AS language \
             FROM information_schema.routines \
             WHERE routine_schema = $1 \
             ORDER BY routine_name",
            &[&schema],
        )
        .await?;
    output::print_json(&output::rows_to_json(&rows));
    Ok(())
}
