use clap::Parser;
use clap::Subcommand;
use pgdr::commands::connections;
use pgdr::commands::constraint;
use pgdr::commands::db;
use pgdr::commands::function;
use pgdr::commands::graph;
use pgdr::commands::index;
use pgdr::commands::locks;
use pgdr::commands::queries;
use pgdr::commands::query;
use pgdr::commands::role;
use pgdr::commands::schema;
use pgdr::commands::sequence;
use pgdr::commands::server;
use pgdr::commands::table;
use pgdr::commands::view;
use pgdr::output;
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
    #[command(about = "List active connections from pg_stat_activity")]
    Connections {
        #[arg(long)]
        state: Option<String>,
        #[arg(long)]
        database: Option<String>,
        #[arg(long)]
        exclude_internal: bool,
    },
    #[command(about = "List locks from pg_locks")]
    Locks {
        #[arg(long, conflicts_with = "blocked")]
        granted: bool,
        #[arg(long)]
        blocked: bool,
        #[arg(long)]
        schema: Option<String>,
        #[arg(long)]
        relation: Option<String>,
        #[arg(long)]
        exclude_advisory_locks: bool,
    },
    #[command(about = "List top queries from pg_stat_statements")]
    Queries {
        #[arg(long = "order-by", value_enum, default_value_t = queries::OrderBy::TotalTime)]
        order_by: queries::OrderBy,
        #[arg(long, default_value_t = 50)]
        limit: i64,
        #[arg(long)]
        database: Option<String>,
        #[arg(long)]
        role: Option<String>,
    },
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
        Command::Connections {
            state,
            database,
            exclude_internal,
        } => {
            connections::run(
                &client,
                state.as_deref(),
                database.as_deref(),
                exclude_internal,
            )
            .await
        }
        Command::Locks {
            granted,
            blocked,
            schema,
            relation,
            exclude_advisory_locks,
        } => {
            locks::run(
                &client,
                granted,
                blocked,
                schema.as_deref(),
                relation.as_deref(),
                exclude_advisory_locks,
            )
            .await
        }
        Command::Queries {
            order_by,
            limit,
            database,
            role,
        } => {
            queries::run(
                &client,
                order_by,
                limit,
                database.as_deref(),
                role.as_deref(),
            )
            .await
        }
    };

    match result {
        Ok(value) => output::print_value(&value),
        Err(e) => {
            eprint!("error: {e}");
            let mut source = std::error::Error::source(&e);
            while let Some(err) = source {
                eprint!(": {err}");
                source = err.source();
            }
            eprintln!();
            std::process::exit(1);
        }
    }
}
