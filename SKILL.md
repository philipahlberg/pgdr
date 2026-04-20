---
name: pgdr
description: "How to use the pgdr CLI — a non-interactive PostgreSQL tool that outputs JSON — for database introspection, schema exploration, and data querying. Use this skill whenever you need to inspect a PostgreSQL database using pgdr: listing tables, describing columns, fetching rows, exploring indexes or constraints, running raw SQL, or checking server metadata. Trigger this skill any time pgdr is mentioned or the user asks you to query/inspect a Postgres database in this project."
---

# pgdr CLI

`pgdr` is a non-interactive PostgreSQL CLI. Every command prints JSON to stdout. Errors go to stderr; exit code is 0 on success, 1 on failure.

## Prerequisites

`DATABASE_URL` must be set in the environment:

```sh
export DATABASE_URL="postgres://user:password@host:5432/dbname"
```

All commands below assume this is set.

## Commands

### databases

```sh
pgdr databases list
```

Lists all databases with `name`, `encoding`, `collation`, and `character_type`.

---

### schemas

```sh
pgdr schemas list
```

Lists all schemas with `name` and `owner`.

---

### tables

```sh
pgdr tables list [--schema <schema>]
pgdr tables view <table> [--schema <schema>]
pgdr tables get <table> [--schema <schema>] [--limit <n>]
```

- `list` — table names and types in the schema
- `view` — columns with `name`, `type`, `nullable`, `default`, `max_length`, `numeric_precision`, `numeric_scale`
- `get` — actual rows as JSON; use `--limit` to cap the number returned

Default schema is `public` for all three.

---

### views

```sh
pgdr views list [--schema <schema>]
```

Lists views with their SQL definitions.

---

### sequences

```sh
pgdr sequences list [--schema <schema>]
```

Lists sequences with data type, min/max values, and increment.

---

### functions

```sh
pgdr functions list [--schema <schema>]
pgdr functions view <function> [--schema <schema>]
```

- `list` — functions and procedures with `name`, `type`, `return_type`, and `language`
- `view <function>` — a single object with `name`, `schema`, `kind` (`function`/`procedure`/`aggregate`/`window`), `language`, `return_type`, `arguments`, `volatility`, `strict`, `security_definer`, `parallel`, `owner`, `source`, `definition`, and `dependencies`. For SQL- and PL/pgSQL-language functions, `dependencies` is an array of `{kind, schema, name}` resolved by parsing the function body via the PostgreSQL AST (`kind` is one of `function`, `table`, `view`, `materialized_view`, `sequence`, or `foreign_table`; does not track dynamically constructed queries like `EXECUTE '...' || var`). For other languages `dependencies` is `null`. Returns `null` if the function doesn't exist.

---

### indices

```sh
pgdr indices list [--schema <schema>] [--table <table>]
```

Lists indices with `name`, `table`, `unique`, `primary`, `method`, and `definition`. Use `--table` to filter to a specific table.

---

### constraints

```sh
pgdr constraints list [--schema <schema>] [--table <table>]
```

Lists constraints with `name`, `table`, `type`, `update_rule`, `delete_rule`, `foreign_table`, and `foreign_column`. Use `--table` to filter.

---

### query

```sh
pgdr query "<sql>"
```

Runs arbitrary SQL and returns rows as JSON. This is the escape hatch for anything the structured commands don't cover.

```sh
pgdr query "SELECT count(*) FROM orders WHERE status = 'pending'"
```

---

### server

```sh
pgdr server version
pgdr server settings
pgdr server extensions
```

- `version` — PostgreSQL version string
- `settings` — all `pg_settings` rows: `name`, `setting`, `unit`, `category`, `description`, `source`, `default`
- `extensions` — available extensions with `name`, `default_version`, `installed_version`, `description`

---

### roles

```sh
pgdr roles list
pgdr roles view <role>
```

- `list` — all roles with `name`, `superuser`, `create_db`, `create_role`, `can_login`, `replication`, `connection_limit`
- `view <role>` — a single object with the role's attributes (`superuser`, `inherit`, `create_role`, `create_db`, `can_login`, `replication`, `bypass_rls`, `connection_limit`, `valid_until`), role-level `config` settings as a key/value object (or `null`), `member_of` (roles this role belongs to), and `members` (roles that belong to this role). Returns `null` if the role doesn't exist.

---

### connections

```sh
pgdr connections [--state <state>] [--database <name>] [--exclude-internal]
```

Lists sessions from `pg_stat_activity` with `pid`, `leader_pid`, `backend_type`, `database`, `user`, `application_name`, `client_addr`, `client_hostname`, `client_port`, `backend_start`, `xact_start`, `query_start`, `state_change`, `state`, `wait_event_type`, `wait_event`, `backend_xid`, `backend_xmin`, `query_id`, `query`. By default includes internal backend types (autovacuum, walwriter, checkpointer, …); `--exclude-internal` restricts to `client backend` and `parallel worker`.

---

### locks

```sh
pgdr locks [--granted | --blocked] [--schema <schema>] [--relation <name>] [--exclude-advisory-locks]
```

Lists rows from `pg_locks` joined with `pg_class`/`pg_namespace`/`pg_database`/`pg_stat_activity`. Each row has `locktype`, `mode`, `granted`, `fastpath`, `wait_start`, `database`, `schema`, `relation` (nulls for non-relation locks), `pid`, `state`, `query`, and `blocked_by` (array of pids from `pg_blocking_pids`). Advisory locks are included by default; use `--exclude-advisory-locks` to omit. `--granted` and `--blocked` are mutually exclusive.

---

### queries

```sh
pgdr queries [--order-by <key>] [--limit <n>] [--database <name>] [--role <name>]
```

Lists top queries from the `pg_stat_statements` extension. Errors if the extension is not installed. `--order-by` ∈ `total_time` (default) \| `mean_time` \| `calls` \| `rows` \| `io`. `--limit` defaults to 50. Each row has:

- `queryid`, `database`, `role`, `toplevel`, `query`, `calls`, `rows`
- `plan`: `count`, `total_ms`, `min_ms`, `max_ms`, `mean_ms`, `stddev_ms`
- `exec`: `total_ms`, `min_ms`, `max_ms`, `mean_ms`, `stddev_ms`
- `blocks`: `shared`/`local` (each with `hit`, `read`, `dirtied`, `written`) and `temp` (`read`, `written`)
- `io_time_ms`: `blk_read`, `blk_write` (pre-PG17 columns), `shared_read`, `shared_write`, `local_read`, `local_write` (PG17+ columns), `temp_read`, `temp_write` (PG15+). Fields absent on the running server are `null`.
- `wal`: `records`, `fpi`, `bytes`
- `jit`: `functions`, `generation_time_ms`, `inlining_time_ms`, `optimization_time_ms`, `emission_time_ms`

Minimum PostgreSQL version: 14.

---

### graph

```sh
pgdr graph [<pattern>...]
```

Exports the full dependency graph of all database objects as a flat array of directed edges. Each edge has a `dependent` object and a `dependency` object, each with `kind`, `schema`, `name`, and `oid`. System schemas (`pg_catalog`, `information_schema`, `pg_toast`) are excluded.

Optional positional `pattern` arguments filter edges using psql-style globs (`*` matches any sequence, `?` matches a single character — `_` is literal, not a wildcard). An unqualified pattern (`users`) matches the `name`; a qualified pattern (`public.users`) matches `schema` and `name` independently. An edge is included if any pattern matches either the dependent or the dependency. With no patterns, all edges are returned.

```sh
pgdr graph users                 # any edge touching a node named "users"
pgdr graph "public.*"            # any edge touching a node in schema "public"
pgdr graph "*_log" "orders"      # edges touching names ending in "_log" OR named "orders"
pgdr graph "analytics.events_?"  # single-char wildcard
```

Sources combined:
- Views and materialized views → their referenced tables/views/functions (via `pg_depend`)
- SQL-language functions → their referenced tables/views/functions (via `pg_depend`)
- PL/pgSQL (and other procedural) functions → referenced objects extracted by parsing the function body AST
- Tables → tables they reference via foreign keys (via `pg_constraint`)
- Triggers → the table they fire on and the function they call (via `pg_trigger`)

**Finding unused objects** — nodes that never appear as `to_*` are not referenced by anything:

```sh
pgdr graph | jq '
  . as $edges
  | [.[].dependency.name] as $referenced
  | [ $edges[] | select(.dependent.name | IN($referenced[]) | not) | .dependent ]
  | unique'
```

---

## Working with the output

All output is pretty-printed JSON. Pipe to `jq` for filtering and transformation:

```sh
# Get all nullable columns in the users table
pgdr tables view users | jq '[.[] | select(.nullable == true) | .name]'

# Count tables in a schema
pgdr tables list | jq 'length'

# Find foreign key constraints
pgdr constraints list | jq '[.[] | select(.type == "FOREIGN KEY")]'

# Extract a specific setting value
pgdr server settings | jq '.[] | select(.name == "max_connections") | .setting'
```

## Common workflows

**Explore an unfamiliar database:**
```sh
pgdr schemas list
pgdr tables list
pgdr tables view <interesting_table>
pgdr constraints list --table <interesting_table>
pgdr indices list --table <interesting_table>
```

**Sample data from a table:**
```sh
pgdr tables get users --limit 5
```

**Use a non-public schema:**
```sh
pgdr tables list --schema analytics
pgdr tables view events --schema analytics
```
