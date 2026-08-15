# Repository Guidelines

## Project Structure & Module Organization

This repository is a small Rust command-line Todo application.

- `src/main.rs`: application entry point and CLI command dispatch.
- `src/cli.rs`: `clap` command-line argument definitions.
- `src/model.rs`: shared `Todo` data model.
- `src/storage.rs`: SQLite connection, schema initialization, and CRUD operations.
- `src/tui.rs`: `ratatui`/`crossterm` terminal interface and input handling.
- `Cargo.toml` and `Cargo.lock`: dependency and reproducible build configuration.

There are no checked-in assets. Runtime data is stored outside the repository at the user data directory, typically `~/.local/share/todo/todos.db`.

## Build, Test, and Development Commands

Run commands from the repository root:

```bash
cargo run -- -n "Buy milk"    # Add a Todo
cargo run -- -l               # List unfinished Todos
cargo run                     # Open the TUI
cargo fmt --check             # Verify formatting
cargo test                    # Run unit tests
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release         # Build the standalone binary
```

Use `cargo fmt` only when intentionally applying formatting changes. The release binary is written to `target/release/todo`.

## Coding Style & Naming Conventions

Use standard Rust formatting with four-space indentation and run `cargo fmt`. Follow idiomatic Rust naming: `snake_case` for functions and variables, `PascalCase` for types, and `SCREAMING_SNAKE_CASE` only for constants. Keep storage, model, CLI, and TUI responsibilities separated; avoid unrelated refactors in feature changes. Add context to fallible operations with `anyhow` where useful.

## Testing Guidelines

Unit tests currently live alongside the storage implementation in `src/storage.rs`. Test names should describe behavior, for example `persists_add_toggle_and_remove`. Any storage change should cover initialization, persistence across reopen, and invalid input. Run `cargo test` and Clippy before submitting changes.

## Commit & Pull Request Guidelines

Use concise Conventional Commit-style messages, consistent with the existing history: `feat: ...`, `fix: ...`, or `refactor: ...`. Keep each commit focused.

Pull requests should explain the user-visible change, mention relevant commands used for validation, and call out database schema or data-location changes. Include terminal screenshots or recordings when changing TUI behavior. Do not commit `target/`, local databases, editor settings, or secrets.
