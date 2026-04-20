#![cfg_attr(test, allow(unused_crate_dependencies))]

use tokio_postgres::Client;
use tokio_postgres::NoTls;

/// All PG versions with `pg_stat_statements` preloaded, mapped to the port
/// exposed by `docker-compose.yml`.
pub const VERSIONS: &[(&str, u16)] = &[
    ("14", 5414),
    ("15", 5415),
    ("16", 5416),
    ("17", 5417),
    ("18", 5418),
];

/// A PG17 instance without `pg_stat_statements` loaded — used to exercise the
/// "extension not installed" error path.
pub const BARE_PORT: u16 = 5400;

/// Connects to a local Postgres on the given port. Returns `None` if the
/// container isn't reachable so tests can skip gracefully when the compose
/// stack isn't up.
pub async fn try_connect(port: u16) -> Option<Client> {
    let url = format!("postgres://postgres:postgres@localhost:{port}/postgres");
    let (client, connection) = tokio_postgres::connect(&url, NoTls).await.ok()?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    Some(client)
}
