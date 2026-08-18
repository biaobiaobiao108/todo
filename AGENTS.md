# 项目指南 (Repository Guidelines)

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

## 项目结构与模块划分 (Project Structure & Module Organization)

本项目是一个轻量级的 Rust 命令行与终端全屏界面（CLI & TUI）Todo 待办事项应用。

- `src/main.rs`：应用程序入口与 CLI 命令分发。
- `src/cli.rs`：基于 `clap` 的命令行参数定义。
- `src/model.rs`：共享的 `Todo` 数据模型。
- `src/storage.rs`：SQLite 连接管理、数据表结构初始化及 CRUD 操作。
- `src/tui.rs`：基于 `ratatui` 与 `crossterm` 的终端用户界面及交互逻辑。
- `Cargo.toml` 与 `Cargo.lock`：依赖项与可复现构建配置。

仓库内不提交运行时数据文件。运行时数据存储在用户的本地数据目录中，Linux/macOS 下通常位于 `~/.local/share/todo/todos.db`。

---

## 构建、测试与常用命令 (Build, Test, and Development Commands)

请在项目根目录下执行以下命令：

```bash
cargo run -- -n "买牛奶"      # 快速添加一条待办
cargo run -- -l               # 快速列出未完成待办
cargo run                     # 打开全屏 TUI 交互界面
cargo fmt --check             # 检查代码格式
cargo test                    # 运行单元测试
cargo clippy --all-targets --all-features -- -D warnings # 运行 Clippy 静态检查
cargo build --release         # 构建优化后的 Release 单文件二进制
cargo install --path .        # 将最新二进制安装/覆盖至 ~/.cargo/bin/
```

编译生成的 release 二进制位于 `target/release/todo`。

---

## 编码规范与命名约定 (Coding Style & Naming Conventions)

- 遵循标准的 Rust 格式化规范（4 空格缩进），在提交前运行 `cargo fmt`。
- 遵循惯用的 Rust 命名规范：函数与变量使用 `snake_case`，类型使用 `PascalCase`，常量使用 `SCREAMING_SNAKE_CASE`。
- 保持各层职责清晰分离（Storage、Model、CLI 与 TUI），避免在实现功能特性时引入无关的代码重构。
- 对可能产生错误的操作，使用 `anyhow` 提供清晰丰富的上下文错误信息。

---

## 测试准则 (Testing Guidelines)

- 单元测试目前位于 `src/storage.rs` 中。
- 测试用例名称应当准确描述测试行为（例如 `persists_add_toggle_and_remove`、`reloads_external_changes`）。
- 针对存储层的任何修改，都应覆盖初始化、跨实例重新打开的数据持久性、无效输入处理以及外部数据重载。
- 提交代码前必须确保 `cargo test` 与 `cargo clippy` 均完全通过。

---

## 提交与 PR 规范 (Commit & Pull Request Guidelines)

- 使用简洁、规范且符合 Conventional Commits 标准的提交信息，例如：`feat: ...`、`fix: ...` 或 `refactor: ...`，保持每次提交专注且独立。
- 提交说明应解释用户可见的变更，提及用于验证的相关命令，并说明数据库 Schema 或数据存储位置的任何变更。
- 若修改了 TUI 交互行为，建议说明视觉与按键行为变化。
- 严禁将 `target/` 目录、本地测试数据库文件、个人编辑器配置或密钥提交至仓库中。
