mod cli;
mod model;
mod storage;
mod tui;

use std::io::{self, BufRead};

use anyhow::{Result, bail};
use clap::Parser;
use serde_json::json;

use cli::{Cli, Commands};
use model::Todo;
use storage::TodoStore;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut store = TodoStore::load()?;

    if let Some(title) = cli.new_title {
        let todo = store.add(title)?;
        if cli.json {
            println!("{}", serde_json::to_string_pretty(todo)?);
        } else {
            println!("✅ 已添加待办 #{}: {}", todo.id, todo.title);
        }
        return Ok(());
    }

    if cli.list {
        let items: Vec<&Todo> = store
            .items()
            .iter()
            .filter(|item| !item.completed)
            .collect();
        output_todo_list(&items, cli.json, "未完成待办清单", Some(10));
        return Ok(());
    }

    match cli.command {
        Some(Commands::Add { titles }) => {
            let mut added = Vec::new();
            if titles.len() == 1 && titles[0] == "-" {
                let stdin = io::stdin();
                for line in stdin.lock().lines() {
                    let line = line?;
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        let todo = store.add(trimmed.to_string())?.clone();
                        added.push(todo);
                    }
                }
            } else {
                for title in titles {
                    let trimmed = title.trim();
                    if !trimmed.is_empty() {
                        let todo = store.add(trimmed.to_string())?.clone();
                        added.push(todo);
                    }
                }
            }

            if added.is_empty() {
                bail!("未添加任何有效待办内容");
            }

            if cli.json {
                if added.len() == 1 {
                    println!("{}", serde_json::to_string_pretty(&added[0])?);
                } else {
                    println!("{}", serde_json::to_string_pretty(&added)?);
                }
            } else {
                for item in &added {
                    println!("✅ 已添加待办 #{}: {}", item.id, item.title);
                }
            }
        }
        Some(Commands::Done { ids }) => {
            let mut updated = Vec::new();
            for id in ids {
                let item = store.set_completed_by_id(id, true)?.clone();
                updated.push(item);
            }
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&updated)?);
            } else {
                for item in &updated {
                    println!("✅ 已标记完成待办 #{}: {}", item.id, item.title);
                }
            }
        }
        Some(Commands::Undo { ids }) => {
            let mut updated = Vec::new();
            for id in ids {
                let item = store.set_completed_by_id(id, false)?.clone();
                updated.push(item);
            }
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&updated)?);
            } else {
                for item in &updated {
                    println!("⏳ 已重新设为进行中 #{}: {}", item.id, item.title);
                }
            }
        }
        Some(Commands::Rm { ids }) => {
            let mut removed = Vec::new();
            for id in ids {
                let item = store.remove_by_id(id)?;
                removed.push(item);
            }
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&removed)?);
            } else {
                for item in &removed {
                    println!("🗑️ 已删除待办 #{}: {}", item.id, item.title);
                }
            }
        }
        Some(Commands::Edit { id, title }) => {
            let item = store.update_title_by_id(id, title)?.clone();
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&item)?);
            } else {
                println!("✏️ 已修改待办 #{}: {}", item.id, item.title);
            }
        }
        Some(Commands::List { all, done, limit }) => {
            let mut items: Vec<&Todo> = store
                .items()
                .iter()
                .filter(|item| {
                    if all {
                        true
                    } else if done {
                        item.completed
                    } else {
                        !item.completed
                    }
                })
                .collect();

            // 排序：未完成项创建时间倒序，已完成项完成时间倒序
            items.sort_by_key(|b| {
                if b.completed {
                    std::cmp::Reverse(b.completed_at.unwrap_or(b.created_at))
                } else {
                    std::cmp::Reverse(b.created_at)
                }
            });

            let title = if all {
                "全部待办事项清单"
            } else if done {
                "已完成待办归档清单"
            } else {
                "未完成待办事项清单"
            };

            output_todo_list(&items, cli.json, title, limit);
        }
        Some(Commands::Search { keyword }) => {
            let items = store.search(&keyword);
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&items)?);
            } else if items.is_empty() {
                println!("🔍 未找到包含 \"{keyword}\" 的待办事项");
            } else {
                println!("🔍 搜索 \"{keyword}\" 的结果 (共 {} 项):", items.len());
                println!("──────────────────────────────────────────────────");
                for item in &items {
                    let icon = if item.completed { "✅" } else { "⬜" };
                    let time_str = item.created_at.format("%m-%d %H:%M").to_string();
                    println!(
                        "  {} [{}] {} (ID: #{})",
                        icon, time_str, item.title, item.id
                    );
                }
                println!("──────────────────────────────────────────────────");
            }
        }
        Some(Commands::Clear { done: _ }) => {
            let count = store.clear_completed()?;
            if cli.json {
                println!("{}", json!({ "cleared": count }));
            } else {
                println!("🧹 已清理 {count} 条已完成的待办记录");
            }
        }
        Some(Commands::Stats) => {
            let pending = store.items().iter().filter(|i| !i.completed).count();
            let completed = store.items().len() - pending;
            let total = store.items().len();

            if cli.json {
                println!(
                    "{}",
                    json!({
                        "pending": pending,
                        "completed": completed,
                        "total": total,
                    })
                );
            } else {
                println!("📊 待办事项统计概览:");
                println!("──────────────────────────────────────────────────");
                println!("  ⏳ 进行中:   {} 项", pending);
                println!("  ✅ 已完成:   {} 项", completed);
                println!("  📑 待办总数: {} 项", total);
                println!("──────────────────────────────────────────────────");
            }
        }
        None => {
            tui::run(&mut store)?;
        }
    }

    Ok(())
}

fn output_todo_list(items: &[&Todo], is_json: bool, title: &str, limit: Option<usize>) {
    let total = items.len();
    let display_items: Vec<_> = if let Some(lim) = limit {
        items.iter().take(lim).copied().collect()
    } else {
        items.to_vec()
    };

    if is_json {
        let json_output = serde_json::to_string_pretty(&display_items).unwrap_or_default();
        println!("{json_output}");
        return;
    }

    if total == 0 {
        println!("🎉 暂无相关待办事项。使用 `todo add \"内容\"` 或 `todo` 查看与管理！");
        return;
    }

    if let Some(lim) = limit
        && total > lim
    {
        println!("📋 {title} (前 {lim} 条 / 共 {total} 项):");
    } else {
        println!("📋 {title} (共 {total} 项):");
    }
    println!("──────────────────────────────────────────────────");
    for (idx, item) in display_items.iter().enumerate() {
        let icon = if item.completed { "✅" } else { "⬜" };
        let time_str = item.created_at.format("%m-%d %H:%M").to_string();
        println!(
            "  {}. [{}] {} {} (ID: #{})",
            idx + 1,
            time_str,
            icon,
            item.title,
            item.id
        );
    }
    println!("──────────────────────────────────────────────────");
}
