# Repository Guidelines
## 铁律规则 (Ironclad Rules)

1. **代码修改后自动提交并推送 Git**：
   - 每次完成代码修改、功能新增或 Bug 修复并通过测试构建（`cargo test`、`cargo clippy` 与 `cargo build --release`）后，必须立即自动执行一次规范清晰的 `git commit`，并将当前分支 `git push` 到其远程跟踪分支。
   - 禁止自动强制推送（`git push -f`）；如果远程未配置、没有跟踪分支或推送失败，必须保留本地提交并明确告知用户。
2. **每次编写完代码自动安装到本地**：
   - 每次完成代码修改并验证通过后，必须自动执行 `cargo install --path .` 将最新二进制安装/替换到本地 `~/.cargo/bin/`，确保本地全局命令始终保持最新。
3. **包管理器优先**：
   - 必须优先使用 `pnpm` 进行依赖安装与脚本执行，`npm` 仅作极端情况兜底。
4. **安全操作**：
   - 运行任何破坏性命令（包括但不限于删除关键文件、重置数据库结构、强制清空存储等）前，必须向用户明确说明风险并获得确认。
5. **最小改动原则**：
   - 修改已有文件时只改动和当前任务直接相关的部分，不擅自进行大范围重构、改写无关组件风格或删除已有逻辑。

---

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
