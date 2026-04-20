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
        #[arg(long)]
        table: Option<String>,
    },
}

pub async fn run(cmd: Command, client: &Client) -> Result<Value> {
    match cmd {
        Command::List { schema, table } => list(client, &schema, table.as_deref()).await,
    }
}

async fn list(client: &Client, schema: &str, table: Option<&str>) -> Result<Value> {
    let rows = if let Some(t) = table {
        client
            .query(
                "SELECT tc.constraint_name AS name, tc.table_name AS table, \
                 tc.constraint_type AS type, \
                 rc.update_rule, rc.delete_rule, \
                 ccu.table_name AS foreign_table, \
                 ccu.column_name AS foreign_column \
                 FROM information_schema.table_constraints tc \
                 LEFT JOIN information_schema.referential_constraints rc \
                   ON rc.constraint_name = tc.constraint_name \
                   AND rc.constraint_schema = tc.constraint_schema \
                 LEFT JOIN information_schema.constraint_column_usage ccu \
                   ON ccu.constraint_name = tc.constraint_name \
                   AND ccu.constraint_schema = tc.constraint_schema \
                 WHERE tc.constraint_schema = $1 AND tc.table_name = $2 \
                 ORDER BY tc.constraint_name",
                &[&schema, &t],
            )
            .await?
    } else {
        client
            .query(
                "SELECT tc.constraint_name AS name, tc.table_name AS table, \
                 tc.constraint_type AS type, \
                 rc.update_rule, rc.delete_rule, \
                 ccu.table_name AS foreign_table, \
                 ccu.column_name AS foreign_column \
                 FROM information_schema.table_constraints tc \
                 LEFT JOIN information_schema.referential_constraints rc \
                   ON rc.constraint_name = tc.constraint_name \
                   AND rc.constraint_schema = tc.constraint_schema \
                 LEFT JOIN information_schema.constraint_column_usage ccu \
                   ON ccu.constraint_name = tc.constraint_name \
                   AND ccu.constraint_schema = tc.constraint_schema \
                 WHERE tc.constraint_schema = $1 \
                 ORDER BY tc.table_name, tc.constraint_name",
                &[&schema],
            )
            .await?
    };
    Ok(Value::Array(output::rows_to_json(&rows)))
}
