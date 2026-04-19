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
    Describe {
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

pub async fn run(cmd: Command, client: &Client) -> Result<()> {
    match cmd {
        Command::List { schema } => list(client, &schema).await,
        Command::Describe { table, schema } => describe(client, &schema, &table).await,
        Command::Get {
            table,
            schema,
            limit,
        } => get(client, &schema, &table, limit).await,
    }
}

async fn list(client: &Client, schema: &str) -> Result<()> {
    let rows = client
        .query(
            "SELECT table_name AS name, table_type AS type \
             FROM information_schema.tables \
             WHERE table_schema = $1 \
             ORDER BY table_name",
            &[&schema],
        )
        .await?;
    output::print_json(&output::rows_to_json(&rows));
    Ok(())
}

async fn describe(client: &Client, schema: &str, table: &str) -> Result<()> {
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
    output::print_json(&output::rows_to_json(&rows));
    Ok(())
}

async fn get(client: &Client, schema: &str, table: &str, limit: Option<i64>) -> Result<()> {
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
    output::print_json(&output::rows_to_json(&rows));
    Ok(())
}
