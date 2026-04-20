use crate::error::Error;
use crate::error::Result;
use crate::output;
use clap::ValueEnum;
use serde_json::Map;
use serde_json::Value;
use tokio_postgres::Client;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OrderBy {
    #[value(name = "total_time")]
    TotalTime,
    #[value(name = "mean_time")]
    MeanTime,
    #[value(name = "calls")]
    Calls,
    #[value(name = "rows")]
    Rows,
    #[value(name = "io")]
    Io,
}

pub async fn run(
    client: &Client,
    order_by: OrderBy,
    limit: i64,
    database: Option<&str>,
    role: Option<&str>,
) -> Result<Value> {
    let installed = client
        .query_opt(
            "SELECT 1 FROM pg_extension WHERE extname = 'pg_stat_statements'",
            &[],
        )
        .await?;
    if installed.is_none() {
        return Err(Error::Message(
            "pg_stat_statements extension is not installed".into(),
        ));
    }

    let version: i32 = client
        .query_one("SELECT current_setting('server_version_num')::int4", &[])
        .await?
        .get(0);

    let (blk_read, blk_write, shared_read, shared_write) = if version >= 170000 {
        (
            "NULL::float8",
            "NULL::float8",
            "shared_blk_read_time",
            "shared_blk_write_time",
        )
    } else {
        (
            "blk_read_time",
            "blk_write_time",
            "NULL::float8",
            "NULL::float8",
        )
    };
    let (local_read, local_write) = if version >= 170000 {
        ("local_blk_read_time", "local_blk_write_time")
    } else {
        ("NULL::float8", "NULL::float8")
    };
    let (temp_read, temp_write) = if version >= 150000 {
        ("temp_blk_read_time", "temp_blk_write_time")
    } else {
        ("NULL::float8", "NULL::float8")
    };
    let (jit_functions, jit_gen, jit_inline, jit_opt, jit_emit) = if version >= 150000 {
        (
            "jit_functions",
            "jit_generation_time",
            "jit_inlining_time",
            "jit_optimization_time",
            "jit_emission_time",
        )
    } else {
        (
            "NULL::int8",
            "NULL::float8",
            "NULL::float8",
            "NULL::float8",
            "NULL::float8",
        )
    };

    let order_col = match order_by {
        OrderBy::TotalTime => "total_exec_time",
        OrderBy::MeanTime => "mean_exec_time",
        OrderBy::Calls => "calls",
        OrderBy::Rows => "rows",
        OrderBy::Io => "(shared_blks_read + shared_blks_written)",
    };

    let sql = format!(
        "SELECT \
         pss.queryid::text AS queryid, \
         d.datname AS database, \
         r.rolname AS role, \
         pss.toplevel, \
         pss.query, \
         pss.calls, \
         pss.rows, \
         pss.plans, \
         pss.total_plan_time, pss.min_plan_time, pss.max_plan_time, pss.mean_plan_time, pss.stddev_plan_time, \
         pss.total_exec_time, pss.min_exec_time, pss.max_exec_time, pss.mean_exec_time, pss.stddev_exec_time, \
         pss.shared_blks_hit, pss.shared_blks_read, pss.shared_blks_dirtied, pss.shared_blks_written, \
         pss.local_blks_hit, pss.local_blks_read, pss.local_blks_dirtied, pss.local_blks_written, \
         pss.temp_blks_read, pss.temp_blks_written, \
         {blk_read} AS blk_read_time, {blk_write} AS blk_write_time, \
         {shared_read} AS shared_blk_read_time, {shared_write} AS shared_blk_write_time, \
         {local_read} AS local_blk_read_time, {local_write} AS local_blk_write_time, \
         {temp_read} AS temp_blk_read_time, {temp_write} AS temp_blk_write_time, \
         pss.wal_records, pss.wal_fpi, pss.wal_bytes::float8 AS wal_bytes, \
         {jit_functions} AS jit_functions, \
         {jit_gen} AS jit_generation_time, \
         {jit_inline} AS jit_inlining_time, \
         {jit_opt} AS jit_optimization_time, \
         {jit_emit} AS jit_emission_time \
         FROM pg_stat_statements pss \
         LEFT JOIN pg_database d ON d.oid = pss.dbid \
         LEFT JOIN pg_roles r ON r.oid = pss.userid \
         WHERE ($1::text IS NULL OR d.datname = $1) \
         AND ($2::text IS NULL OR r.rolname = $2) \
         ORDER BY {order_col} DESC NULLS LAST \
         LIMIT $3"
    );

    let rows = client.query(&sql, &[&database, &role, &limit]).await?;

    let flat_rows = output::rows_to_json(&rows);
    let nested: Vec<Value> = flat_rows.into_iter().map(nest_row).collect();
    Ok(Value::Array(nested))
}

fn nest_row(value: Value) -> Value {
    let Value::Object(mut m) = value else {
        return value;
    };
    let mut take = |k: &str| m.remove(k).unwrap_or(Value::Null);

    let mut out = Map::new();
    out.insert("queryid".into(), take("queryid"));
    out.insert("database".into(), take("database"));
    out.insert("role".into(), take("role"));
    out.insert("toplevel".into(), take("toplevel"));
    out.insert("query".into(), take("query"));
    out.insert("calls".into(), take("calls"));
    out.insert("rows".into(), take("rows"));

    let mut plan = Map::new();
    plan.insert("count".into(), take("plans"));
    plan.insert("total_ms".into(), take("total_plan_time"));
    plan.insert("min_ms".into(), take("min_plan_time"));
    plan.insert("max_ms".into(), take("max_plan_time"));
    plan.insert("mean_ms".into(), take("mean_plan_time"));
    plan.insert("stddev_ms".into(), take("stddev_plan_time"));
    out.insert("plan".into(), Value::Object(plan));

    let mut exec = Map::new();
    exec.insert("total_ms".into(), take("total_exec_time"));
    exec.insert("min_ms".into(), take("min_exec_time"));
    exec.insert("max_ms".into(), take("max_exec_time"));
    exec.insert("mean_ms".into(), take("mean_exec_time"));
    exec.insert("stddev_ms".into(), take("stddev_exec_time"));
    out.insert("exec".into(), Value::Object(exec));

    let mut shared = Map::new();
    shared.insert("hit".into(), take("shared_blks_hit"));
    shared.insert("read".into(), take("shared_blks_read"));
    shared.insert("dirtied".into(), take("shared_blks_dirtied"));
    shared.insert("written".into(), take("shared_blks_written"));

    let mut local = Map::new();
    local.insert("hit".into(), take("local_blks_hit"));
    local.insert("read".into(), take("local_blks_read"));
    local.insert("dirtied".into(), take("local_blks_dirtied"));
    local.insert("written".into(), take("local_blks_written"));

    let mut temp = Map::new();
    temp.insert("read".into(), take("temp_blks_read"));
    temp.insert("written".into(), take("temp_blks_written"));

    let mut blocks = Map::new();
    blocks.insert("shared".into(), Value::Object(shared));
    blocks.insert("local".into(), Value::Object(local));
    blocks.insert("temp".into(), Value::Object(temp));
    out.insert("blocks".into(), Value::Object(blocks));

    let mut io = Map::new();
    io.insert("blk_read".into(), take("blk_read_time"));
    io.insert("blk_write".into(), take("blk_write_time"));
    io.insert("shared_read".into(), take("shared_blk_read_time"));
    io.insert("shared_write".into(), take("shared_blk_write_time"));
    io.insert("local_read".into(), take("local_blk_read_time"));
    io.insert("local_write".into(), take("local_blk_write_time"));
    io.insert("temp_read".into(), take("temp_blk_read_time"));
    io.insert("temp_write".into(), take("temp_blk_write_time"));
    out.insert("io_time_ms".into(), Value::Object(io));

    let mut wal = Map::new();
    wal.insert("records".into(), take("wal_records"));
    wal.insert("fpi".into(), take("wal_fpi"));
    wal.insert("bytes".into(), take("wal_bytes"));
    out.insert("wal".into(), Value::Object(wal));

    let mut jit = Map::new();
    jit.insert("functions".into(), take("jit_functions"));
    jit.insert("generation_time_ms".into(), take("jit_generation_time"));
    jit.insert("inlining_time_ms".into(), take("jit_inlining_time"));
    jit.insert("optimization_time_ms".into(), take("jit_optimization_time"));
    jit.insert("emission_time_ms".into(), take("jit_emission_time"));
    out.insert("jit".into(), Value::Object(jit));

    Value::Object(out)
}
