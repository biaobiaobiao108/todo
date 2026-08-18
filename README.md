# Todo

一个使用 Rust 编写的现代化轻量级终端 Todo 待办事项管理工具。兼具高效的全屏交互式 TUI 界面与适合脚本、流水线及 AI Agent 调用的完整 CLI 工具链。

---

## 🌟 核心特性

- 🚀 **全宽现代 TUI 界面**：自适应全宽待办列表，中文字符光标精准对齐，带底部选中项详情卡片与删除二次确认。
- 🤖 **Agent 与脚本友好**：所有子命令均支持 `--json` 输出结构化数据，方便大模型（LLM）与自动化脚本解析。
- 🔗 **管道与批处理支持**：支持从标准输入（`stdin`）批量录入待办（`cat tasks.txt | todo add -`），支持多 ID 批量完成/删除。
- 📦 **开箱即用持久化**：内置静态编译的 SQLite（bundled），零外部动态库依赖，数据安全持久化于本地。
- ⚡ **毫秒级并发同步**：TUI 运行期间若有外部 CLI 写入，界面自动实时同步刷新。

---

## 📦 安装与配置

需要本地已安装 Rust 工具链。在仓库根目录下直接安装：

```bash
cargo install --path .
```

如果系统提示 `todo` 命令未找到，请确保 Cargo 的 bin 目录已加入环境变量 `PATH`：

```bash
# Bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc

# Zsh
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

也可以直接构建独立的 Release 单文件二进制：

```bash
cargo build --release
./target/release/todo --help
```

---

## 🖥️ TUI 交互界面

直接运行 `todo` 即可进入全屏 TUI 管理界面：

```bash
todo
```

### 快捷键指南

| 快捷键 | 功能操作 |
| :--- | :--- |
| `j` / `k` / `↑` / `↓` | 在待办列表中上下移动选择 |
| `Space` | 切换当前待办的完成/未完成状态 |
| `Enter` / `e` | 进入编辑弹窗，修改当前待办内容 |
| `a` / `n` | 打开新增待办事项弹窗 |
| `d` / `Delete` | 删除当前待办（带确认弹窗） |
| `Tab` | 快速切换 **进行中 (Pending)** / **已完成 (Completed)** 分类 |
| `q` / `Esc` | 退出 TUI 界面 |
| **鼠标点击** | 点击任意行可直接选中；点击左侧复选框区域可直接切换完成状态 |

---

## 💻 CLI 命令行使用说明

### 1. 基础待办操作

```bash
# 快速添加待办
todo add "阅读技术文档"
todo add "买牛奶" "买面包"        # 批量添加多个待办
todo -n "快速语法糖添加"          # 向下兼容语法糖

# 从标准输入批量添加 (Pipeline)
cat task_list.txt | todo add -

# 标记完成与撤销
todo done 1                     # 标记 #1 为完成
todo done 1 2 3                 # 批量标记 #1, #2, #3 为完成
todo undo 1                     # 重新将 #1 设为未完成

# 修改与删除
todo edit 1 "修改后的新标题"
todo rm 1 2                     # 批量删除 #1 和 #2
todo clear                      # 一键清理所有已完成的待办归档
```

### 2. 查询与检索

```bash
# 查看待办清单
todo list                       # 列出当前未完成待办
todo list --all                 # 列出全部待办（含已完成）
todo list --done                # 仅列出已完成待办
todo list --limit 5             # 限制输出前 5 条
todo -l                         # 快捷列出未完成待办

# 关键词搜索
todo search "方案"               # 模糊搜索包含“方案”的待办
```

### 3. 结构化数据输出 (`--json`)

为脚本自动化与 AI Agent 调用提供纯净的 JSON 格式输出：

```bash
# 查询待办列表
todo list --json

# 新增待办并返回新建对象
todo add "部署新版本服务" --json

# 获取统计指标
todo stats --json
# 输出: {"pending": 3, "completed": 12, "total": 15}

# 结合 jq 自动化处理
todo list --json | jq -r '.[] | select(.completed == false) | .title'
```

---

## 🗄️ 数据存储

数据使用 SQLite 本地数据库持久化存储，路径位于：

- **Linux / macOS**: `~/.local/share/todo/todos.db`
- **Windows**: `%APPDATA%\todo\todos.db`

首次启动时会自动初始化目录与数据库 Schema。

---

## 🛠️ 本地开发与测试

```bash
cargo fmt --check                                       # 代码格式检查
cargo test                                              # 运行单元测试
cargo clippy --all-targets --all-features -- -D warnings # 静态分析检查
cargo build --release                                   # 构建 Release 产物
cargo install --path .                                  # 安装覆盖到本地
```

