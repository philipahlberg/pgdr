use crate::error::Result;
use crate::output;
use clap::Subcommand;
use tokio_postgres::Client;

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
            "SELECT sequence_name AS name, data_type, start_value, minimum_value, \
             maximum_value, increment, cycle_option \
             FROM information_schema.sequences \
             WHERE sequence_schema = $1 \
             ORDER BY sequence_name",
            &[&schema],
        )
        .await?;
    output::print_json(&output::rows_to_json(&rows));
    Ok(())
}
