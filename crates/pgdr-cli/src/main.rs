mod commands;
mod error;
mod output;
mod parse;

use crate::commands::constraint;
use crate::commands::db;
use crate::commands::function;
use crate::commands::graph;
use crate::commands::index;
use crate::commands::query;
use crate::commands::role;
use crate::commands::schema;
use crate::commands::sequence;
use crate::commands::server;
use crate::commands::table;
use crate::commands::view;
use clap::Parser;
use clap::Subcommand;
use tokio_postgres::NoTls;

#[derive(Debug, Parser)]
#[command(name = "pgdr", about = "Non-interactive Postgres CLI with JSON output")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(subcommand, about = "List databases")]
    Db(db::Command),
    #[command(subcommand, about = "List schemas")]
    Schema(schema::Command),
    #[command(subcommand, about = "Inspect tables and fetch rows")]
    Table(table::Command),
    #[command(subcommand, about = "List views")]
    View(view::Command),
    #[command(subcommand, about = "List sequences")]
    Sequence(sequence::Command),
    #[command(subcommand, about = "List functions and procedures")]
    Function(function::Command),
    #[command(subcommand, about = "List indexes")]
    Index(index::Command),
    #[command(subcommand, about = "List constraints")]
    Constraint(constraint::Command),
    #[command(about = "Run a SQL query and return rows as JSON")]
    Query { sql: String },
    #[command(about = "Export the full dependency graph of all database objects as edges")]
    Graph {
        /// Name patterns to filter edges (glob-style with `*` and `?`).
        /// Unqualified patterns match the `name`; qualified patterns `schema.name`
        /// match both. An edge is included if either side matches any pattern.
        patterns: Vec<String>,
    },
    #[command(subcommand, about = "Server information")]
    Server(server::Command),
    #[command(subcommand, about = "Inspect roles")]
    Role(role::Command),
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        eprintln!("error: DATABASE_URL is not set");
        std::process::exit(1);
    });
    let (client, connection) = tokio_postgres::connect(&url, NoTls)
        .await
        .unwrap_or_else(|e| {
            eprintln!("error: {e}");
            std::process::exit(1);
        });

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {e}");
        }
    });

    let result = match cli.command {
        Command::Db(cmd) => db::run(cmd, &client).await,
        Command::Schema(cmd) => schema::run(cmd, &client).await,
        Command::Table(cmd) => table::run(cmd, &client).await,
        Command::View(cmd) => view::run(cmd, &client).await,
        Command::Sequence(cmd) => sequence::run(cmd, &client).await,
        Command::Function(cmd) => function::run(cmd, &client).await,
        Command::Index(cmd) => index::run(cmd, &client).await,
        Command::Constraint(cmd) => constraint::run(cmd, &client).await,
        Command::Query { sql } => query::run(&sql, &client).await,
        Command::Graph { patterns } => graph::run(&client, &patterns).await,
        Command::Server(cmd) => server::run(cmd, &client).await,
        Command::Role(cmd) => role::run(cmd, &client).await,
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
