use crate::error::Result;
use crate::output;
use tokio_postgres::Client;

pub async fn run(sql: &str, client: &Client) -> Result<()> {
    let rows = client.query(sql, &[]).await?;
    output::print_json(&output::rows_to_json(&rows));
    Ok(())
}
