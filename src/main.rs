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
        println!("已添加 #{}: {}", todo.id, todo.title);
    } else if cli.list {
        print_pending(&store);
    } else {
        tui::run(&mut store)?;
    }

    Ok(())
}

fn print_pending(store: &TodoStore) {
    let pending: Vec<_> = store
        .items()
        .iter()
        .filter(|item| !item.completed)
        .collect();
    if pending.is_empty() {
        println!("没有未完成的待办事项。🎉");
        return;
    }

    println!("未完成待办（{} 项）", pending.len());
    for item in pending {
        println!(
            "[ ] {:>3}  {}  ({})",
            item.id,
            item.title,
            item.created_at.format("%Y-%m-%d %H:%M")
        );
    }
}
