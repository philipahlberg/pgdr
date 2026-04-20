use crate::error::Result;
use crate::output;
use serde_json::Value;
use tokio_postgres::Client;

pub async fn run(sql: &str, client: &Client) -> Result<Value> {
    let rows = client.query(sql, &[]).await?;
    Ok(Value::Array(output::rows_to_json(&rows)))
}
