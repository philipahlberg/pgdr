---
name: pgdr
description: How to use the pgdr CLI — a non-interactive PostgreSQL tool that outputs JSON — for database introspection, schema exploration, and data querying. Use this skill whenever you need to inspect a PostgreSQL database using pgdr: listing tables, describing columns, fetching rows, exploring indexes or constraints, running raw SQL, or checking server metadata. Trigger this skill any time pgdr is mentioned or the user asks you to query/inspect a Postgres database in this project.
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

### db

```sh
pgdr db list
```

Lists all databases with `name`, `encoding`, `collation`, and `character_type`.

---

### schema

```sh
pgdr schema list
```

Lists all schemas with `name` and `owner`.

---

### table

```sh
pgdr table list [--schema <schema>]
pgdr table describe <table> [--schema <schema>]
pgdr table get <table> [--schema <schema>] [--limit <n>]
```

- `list` — table names and types in the schema
- `describe` — columns with `name`, `type`, `nullable`, `default`, `max_length`, `numeric_precision`, `numeric_scale`
- `get` — actual rows as JSON; use `--limit` to cap the number returned

Default schema is `public` for all three.

---

### view

```sh
pgdr view list [--schema <schema>]
```

Lists views with their SQL definitions.

---

### sequence

```sh
pgdr sequence list [--schema <schema>]
```

Lists sequences with data type, min/max values, and increment.

---

### function

```sh
pgdr function list [--schema <schema>]
```

Lists functions and procedures with `name`, `type`, `return_type`, and `language`.

---

### index

```sh
pgdr index list [--schema <schema>] [--table <table>]
```

Lists indexes with `name`, `table`, `unique`, `primary`, `method`, and `definition`. Use `--table` to filter to a specific table.

---

### constraint

```sh
pgdr constraint list [--schema <schema>] [--table <table>]
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

### role

```sh
pgdr role list
pgdr role describe <role>
```

- `list` — all roles with `name`, `superuser`, `create_db`, `create_role`, `can_login`, `replication`, `connection_limit`
- `describe <role>` — lists which roles this role is a member of

---

## Working with the output

All output is pretty-printed JSON. Pipe to `jq` for filtering and transformation:

```sh
# Get all nullable columns in the users table
pgdr table describe users | jq '[.[] | select(.nullable == true) | .name]'

# Count tables in a schema
pgdr table list | jq 'length'

# Find foreign key constraints
pgdr constraint list | jq '[.[] | select(.type == "FOREIGN KEY")]'

# Extract a specific setting value
pgdr server settings | jq '.[] | select(.name == "max_connections") | .setting'
```

## Common workflows

**Explore an unfamiliar database:**
```sh
pgdr schema list
pgdr table list
pgdr table describe <interesting_table>
pgdr constraint list --table <interesting_table>
pgdr index list --table <interesting_table>
```

**Sample data from a table:**
```sh
pgdr table get users --limit 5
```

**Use a non-public schema:**
```sh
pgdr table list --schema analytics
pgdr table describe events --schema analytics
```
