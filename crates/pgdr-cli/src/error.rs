use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("database error: {0}")]
    Postgres(#[from] tokio_postgres::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("parse error: {0}")]
    Parse(#[from] pg_query::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
