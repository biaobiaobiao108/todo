mod cli;
mod model;
mod storage;
mod tui;

use anyhow::Result;
use clap::Parser;

use cli::Cli;
use storage::TodoStore;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut store = TodoStore::load()?;

    if let Some(title) = cli.new_title {
        let todo = store.add(title)?;
        println!("✅ 已添加待办 #{}: {}", todo.id, todo.title);
    } else if cli.list {
        print_pending(&store);
    } else {
        tui::run(&mut store)?;
    }

    Ok(())
}

fn print_pending(store: &TodoStore) {
    let mut pending: Vec<_> = store
        .items()
        .iter()
        .filter(|item| !item.completed)
        .collect();
    if pending.is_empty() {
        println!("🎉 暂无未完成待办事项。输入 `todo` 进入 TUI 或使用 `todo -n \"内容\"` 快速添加！");
        return;
    }

    let total_pending = pending.len();
    // 按创建时间倒序（最新创建的排在前面）
    pending.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let display_items: Vec<_> = pending.into_iter().take(10).collect();

    if total_pending > 10 {
        println!(
            "📋 最近未完成待办 (最新 10 条 / 共 {} 项):",
            total_pending
        );
    } else {
        println!("📋 未完成待办清单 (共 {} 项):", total_pending);
    }
    println!("──────────────────────────────────────────────────");
    for (idx, item) in display_items.iter().enumerate() {
        let time_str = item.created_at.format("%m-%d %H:%M").to_string();
        println!(
            "  {}. [{}] ⬜ {} (ID: #{})",
            idx + 1,
            time_str,
            item.title,
            item.id
        );
    }
    println!("──────────────────────────────────────────────────");
    println!("💡 提示: 运行 `todo` 进入 TUI 查看全部并管理待办");
}
