# Todo

一个使用 Rust 编写的轻量级终端 Todo 工具。支持命令行快速添加和查看待办，也提供带键盘、鼠标交互的 TUI 界面。

## 功能

- `todo -n "待办内容"` 快速添加待办
- `todo -l` 查看未完成待办
- `todo` 进入 TUI 管理界面
- SQLite 持久化存储，数据不会因为程序退出而丢失
- TUI 中支持新增、完成/恢复、删除和查看详细信息
- 支持鼠标点击待办左侧复选框完成待办

## 安装

需要 Rust 工具链。直接从源码安装：

```bash
cargo install --path .
```

如果 `todo` 命令无法找到，请将 Cargo 的用户级 bin 目录加入 PATH：

```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

也可以只构建 release 二进制：

```bash
cargo build --release
./target/release/todo --help
```

## 使用方法

```bash
todo -n "完成 Rust 项目"
todo -l
todo
```

进入 TUI 后：

| 操作 | 快捷键 |
| --- | --- |
| 移动选择 | `↑` / `↓` |
| 完成或恢复 | `Enter` / `Space` |
| 新增待办 | `a` |
| 删除待办 | `d` |
| 切换未完成/全部 | `Tab` |
| 退出 | `q` / `Esc` |

鼠标点击待办行可以选中待办，只有点击最左侧的 `[ ]` 或 `[✓]` 区域才会切换完成状态。

## 数据存储

程序使用内置 SQLite，数据库默认位于：

```text
~/.local/share/todo/todos.db
```

首次启动会自动创建目录和数据表。旧版本的 `todos.json` 不会被读取或删除。

## 开发

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

SQLite 使用 `bundled` 特性编译，因此生成的 release 二进制不依赖目标机器额外安装 SQLite 动态库。
