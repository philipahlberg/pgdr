use clap::Subcommand;
use tokio_postgres::Client;

use crate::{error::Result, output};

#[derive(Debug, Subcommand)]
pub enum Command {
    List,
    Describe { role: String },
}

pub async fn run(cmd: Command, client: &Client) -> Result<()> {
    match cmd {
        Command::List => list(client).await,
        Command::Describe { role } => describe(client, &role).await,
    }
}

async fn list(client: &Client) -> Result<()> {
    let rows = client
        .query(
            "SELECT rolname AS name, rolsuper AS superuser, \
             rolcreatedb AS create_db, rolcreaterole AS create_role, \
             rolcanlogin AS can_login, rolreplication AS replication, \
             rolconnlimit AS connection_limit \
             FROM pg_roles \
             ORDER BY rolname",
            &[],
        )
        .await?;
    output::print_json(&output::rows_to_json(&rows));
    Ok(())
}

async fn describe(client: &Client, role: &str) -> Result<()> {
    let rows = client
        .query(
            "SELECT r.rolname AS member_of \
             FROM pg_roles r \
             JOIN pg_auth_members m ON m.roleid = r.oid \
             JOIN pg_roles u ON u.oid = m.member \
             WHERE u.rolname = $1 \
             ORDER BY r.rolname",
            &[&role],
        )
        .await?;
    output::print_json(&output::rows_to_json(&rows));
    Ok(())
}
