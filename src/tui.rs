use std::io::{self, stdout};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::storage::TodoStore;

#[derive(Clone, Copy, PartialEq)]
enum Filter {
    Pending,
    Completed,
}

pub fn run(store: &mut TodoStore) -> Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    let result = app(&mut terminal, store);
    disable_raw_mode()?;
    execute!(stdout(), event::DisableMouseCapture, LeaveAlternateScreen)?;
    result
}

fn app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, store: &mut TodoStore) -> Result<()> {
    let mut filter = Filter::Pending;
    let mut selected = 0usize;
    let mut input = None::<String>;
    let mut confirm_delete = false;

    loop {
        terminal.draw(|frame| {
            draw(
                frame,
                store,
                filter,
                selected,
                input.as_deref(),
                confirm_delete,
            )
        })?;
        if !event::poll(std::time::Duration::from_millis(250))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if input.is_some() => match key.code {
                KeyCode::Esc => input = None,
                KeyCode::Enter => {
                    if let Some(title) = input.take()
                        && !title.trim().is_empty()
                    {
                        store.add(title)?;
                    }
                }
                KeyCode::Backspace => {
                    input.as_mut().unwrap().pop();
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    input.as_mut().unwrap().push(c)
                }
                _ => {}
            },
            Event::Key(key) => {
                if confirm_delete {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            if let Some(index) = visible_index(store, filter, selected) {
                                store.remove(index)?;
                            }
                            confirm_delete = false;
                        }
                        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                            confirm_delete = false
                        }
                        _ => {}
                    }
                    continue;
                }
                match key {
                    KeyEvent {
                        code: KeyCode::Char('q'),
                        ..
                    }
                    | KeyEvent {
                        code: KeyCode::Esc, ..
                    } => break,
                    KeyEvent {
                        code: KeyCode::Up, ..
                    } => selected = selected.saturating_sub(1),
                    KeyEvent {
                        code: KeyCode::Down,
                        ..
                    } => {
                        selected = selected
                            .saturating_add(1)
                            .min(visible_len(store, filter).saturating_sub(1))
                    }
                    KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }
                    | KeyEvent {
                        code: KeyCode::Char(' '),
                        ..
                    } => {
                        if let Some(index) = visible_index(store, filter, selected) {
                            store.toggle(index)?;
                        }
                    }
                    KeyEvent {
                        code: KeyCode::Char('a'),
                        ..
                    } => input = Some(String::new()),
                    KeyEvent {
                        code: KeyCode::Char('d'),
                        ..
                    } => {
                        if visible_index(store, filter, selected).is_some() {
                            confirm_delete = true
                        }
                    }
                    KeyEvent {
                        code: KeyCode::Tab, ..
                    } => {
                        filter = if filter == Filter::Pending {
                            Filter::Completed
                        } else {
                            Filter::Pending
                        };
                        selected = 0;
                    }
                    _ => {}
                }
            }
            Event::Mouse(mouse) => {
                if let MouseEventKind::Down(MouseButton::Left) = mouse.kind
                    && let Some((index, checkbox)) = row_at(mouse.row, mouse.column, store, filter)
                {
                    selected = visible_position(store, filter, index);
                    if checkbox {
                        store.toggle(index)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn draw(
    frame: &mut Frame,
    store: &TodoStore,
    filter: Filter,
    selected: usize,
    input: Option<&str>,
    confirm: bool,
) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);
    let pending = store.items().iter().filter(|item| !item.completed).count();
    let completed = store.items().len() - pending;
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " TODO ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("  未完成 {} · 已完成 {}", pending, completed)),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    let visible: Vec<_> = store
        .items()
        .iter()
        .enumerate()
        .filter(|(_, item)| matches_filter(item.completed, filter))
        .collect();
    let items: Vec<ListItem> = visible
        .iter()
        .map(|(_, item)| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    if item.completed { "[✓] " } else { "[ ] " },
                    if item.completed {
                        Color::Green
                    } else {
                        Color::DarkGray
                    },
                ),
                Span::raw(&item.title),
                Span::styled(
                    format!("  #{}", item.id),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .title(if filter == Filter::Pending {
                    " 未完成 "
                } else {
                    " 已完成 "
                })
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("➜ ");
    let mut state = ListState::default();
    if !visible.is_empty() {
        state.select(Some(selected.min(visible.len() - 1)));
    }
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(chunks[1]);
    frame.render_stateful_widget(list, body[0], &mut state);
    let detail = visible.get(selected.min(visible.len().saturating_sub(1)));
    let detail_text = detail
        .map(|(_, item)| {
            format!(
                "标题\n{}\n\n编号\n#{}\n\n创建时间\n{}\n\n状态\n{}{}",
                item.title,
                item.id,
                item.created_at.format("%Y-%m-%d %H:%M"),
                if item.completed {
                    "已完成"
                } else {
                    "未完成"
                },
                item.completed_at
                    .map(|time| format!("\n完成于\n{}", time.format("%Y-%m-%d %H:%M")))
                    .unwrap_or_default()
            )
        })
        .unwrap_or_else(|| "暂无待办\n\n按 a 添加一条新的待办".to_owned());
    frame.render_widget(
        Paragraph::new(detail_text)
            .block(Block::default().title(" 详细信息 ").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        body[1],
    );
    let footer = Paragraph::new("↑↓选择  Enter/空格切换状态  a新增  d删除  Tab切换列表  q退出")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);

    if let Some(value) = input {
        draw_input(frame, value);
    }
    if confirm {
        draw_confirm(frame);
    }
}

fn draw_input(frame: &mut Frame, value: &str) {
    let area = centered(frame.area(), 60, 20);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(value)
            .block(
                Block::default()
                    .title(" 新增待办（Enter保存，Esc取消） ")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
    frame.set_cursor_position((area.x + 1 + value.len() as u16, area.y + 1));
}
fn draw_confirm(frame: &mut Frame) {
    let area = centered(frame.area(), 40, 20);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new("确认删除？按 y 确认，n/Esc 取消")
            .block(Block::default().title(" 删除待办 ").borders(Borders::ALL)),
        area,
    );
}
fn centered(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width.min(area.width)) / 2,
        y: area.y + area.height.saturating_sub(height.min(area.height)) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}
fn visible_len(store: &TodoStore, filter: Filter) -> usize {
    store
        .items()
        .iter()
        .filter(|item| matches_filter(item.completed, filter))
        .count()
}
fn visible_index(store: &TodoStore, filter: Filter, position: usize) -> Option<usize> {
    store
        .items()
        .iter()
        .enumerate()
        .filter(|(_, item)| matches_filter(item.completed, filter))
        .nth(position)
        .map(|(index, _)| index)
}
fn visible_position(store: &TodoStore, filter: Filter, index: usize) -> usize {
    store
        .items()
        .iter()
        .enumerate()
        .filter(|(_, item)| matches_filter(item.completed, filter))
        .position(|(i, _)| i == index)
        .unwrap_or(0)
}

fn matches_filter(completed: bool, filter: Filter) -> bool {
    match filter {
        Filter::Pending => !completed,
        Filter::Completed => completed,
    }
}

fn row_at(row: u16, column: u16, store: &TodoStore, filter: Filter) -> Option<(usize, bool)> {
    if row < 4 {
        return None;
    }
    let position = (row - 4) as usize;
    let index = visible_index(store, filter, position)?;
    // 列表左边框和选中符号占用前两列，随后四列是 `[ ] ` / `[✓] `。
    // 只有点击这个复选框区域才切换完成状态，点击其他位置只选中该行。
    Some((index, (2..=5).contains(&column)))
}
