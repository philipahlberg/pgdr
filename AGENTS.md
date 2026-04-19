# AGENTS.md

This file provides guidance to AI agents working with code in this repository.

## Commands

Tasks are managed via `mise`. Use `mise run <task>` or the underlying cargo commands directly.

| Task | Command |
|------|---------|
| Build (release) | `mise run build` / `cargo build --release --all-features` |
| Check | `mise run check` / `cargo check` |
| Test | `mise run test` / `cargo test` |
| Clippy | `mise run clippy` / `cargo clippy -- -D warnings` |
| Format | `mise run fmt` / `cargo +nightly fmt --all && taplo fmt` |
| Dependency audit | `mise run deny` / `cargo deny check advisories bans licenses` |
| Full CI check | `mise run ci` |
| Auto-fix + test | `mise run fix` |

Run a single test: `cargo test <test_name>`.

## Using the pgdr CLI

See [SKILL.md](SKILL.md) for full documentation on how to use the `pgdr` CLI to inspect PostgreSQL databases.
