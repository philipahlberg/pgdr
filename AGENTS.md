# AGENTS.md

This file provides guidance to AI agents working with code in this repository.

# Workspace

The following is an overview of the contents of this workspace. Make sure to update it if you change anything.

- `SKILL.md` — full CLI reference: `db`, `schema`, `table`, `view`, `sequence`, `function`, `index`, `constraint`, `query`, `server`, `role`, `graph`, plus `jq` workflows
- `Cargo.toml` / `Cargo.lock` — workspace manifest; declare dependencies here and reference them in crates
- `mise.toml` — task runner (`build`, `test`, `fmt`, `clippy`, `deny`, `check`, `ci`, `fix`); loads `.env`
- `rust-toolchain.toml`, `rustfmt.toml`, `taplo.toml` — toolchain + formatter config
- `deny.toml` — `cargo-deny` policy (advisories/bans/licenses)
- `crates/pgdr-cli/` — the sole workspace member, binary `pgdr`
  - `src/main.rs` — clap CLI, connects via `DATABASE_URL`, dispatches to command modules
  - `src/error.rs`, `src/output.rs`, `src/parse.rs` — shared error type, JSON output helpers, SQL/AST parsing utilities
  - `src/commands/` — one module per subcommand: `db`, `schema`, `table`, `view`, `sequence`, `function`, `index`, `constraint`, `query`, `graph`, `server`, `role`, plus `mod.rs`
- `.github/workflows/` — CI: `run_tests.yml`, `run_checks.yml`, `run_build.yml`, and dispatchers `on_pr.yml`, `on_pr_target.yml`, `on_push.yml`, `dispatch_build.yml`; `dependabot.yml` for deps
- `.cargo/config.toml` — cargo overrides
- `.vscode/settings.json` — editor settings
- `.claude/` — local Claude settings
- `target/` — build artifacts (gitignored)

# Commands

Make sure to run `mise fix` when finishing a round of changes.

# Using the pgdr CLI

See [SKILL.md](SKILL.md) for full documentation on how to use the `pgdr` CLI to inspect PostgreSQL databases.
