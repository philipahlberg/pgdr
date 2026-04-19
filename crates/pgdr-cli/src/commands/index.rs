use clap::Subcommand;
use tokio_postgres::Client;

use crate::{error::Result, output};

#[derive(Debug, Subcommand)]
pub enum Command {
    List {
        #[arg(long, default_value = "public")]
        schema: String,
        #[arg(long)]
        table: Option<String>,
    },
}

pub async fn run(cmd: Command, client: &Client) -> Result<()> {
    match cmd {
        Command::List { schema, table } => list(client, &schema, table.as_deref()).await,
    }
}

async fn list(client: &Client, schema: &str, table: Option<&str>) -> Result<()> {
    let rows = if let Some(t) = table {
        client
            .query(
                "SELECT i.relname AS name, t.relname AS table, \
                 ix.indisunique AS unique, ix.indisprimary AS primary, \
                 am.amname AS method, \
                 pg_get_indexdef(ix.indexrelid) AS definition \
                 FROM pg_index ix \
                 JOIN pg_class i ON i.oid = ix.indexrelid \
                 JOIN pg_class t ON t.oid = ix.indrelid \
                 JOIN pg_am am ON am.oid = i.relam \
                 JOIN pg_namespace n ON n.oid = t.relnamespace \
                 WHERE n.nspname = $1 AND t.relname = $2 \
                 ORDER BY i.relname",
                &[&schema, &t],
            )
            .await?
    } else {
        client
            .query(
                "SELECT i.relname AS name, t.relname AS table, \
                 ix.indisunique AS unique, ix.indisprimary AS primary, \
                 am.amname AS method, \
                 pg_get_indexdef(ix.indexrelid) AS definition \
                 FROM pg_index ix \
                 JOIN pg_class i ON i.oid = ix.indexrelid \
                 JOIN pg_class t ON t.oid = ix.indrelid \
                 JOIN pg_am am ON am.oid = i.relam \
                 JOIN pg_namespace n ON n.oid = t.relnamespace \
                 WHERE n.nspname = $1 \
                 ORDER BY t.relname, i.relname",
                &[&schema],
            )
            .await?
    };
    output::print_json(&output::rows_to_json(&rows));
    Ok(())
}
