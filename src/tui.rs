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
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::storage::TodoStore;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    Pending,
    Completed,
}

#[derive(Clone, PartialEq, Eq)]
enum InputState {
    Creating,
    Editing(usize), // store 里的原始索引
}

pub fn run(store: &mut TodoStore) -> Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(
        out,
        EnterAlternateScreen,
        event::EnableMouseCapture,
        crossterm::cursor::SetCursorStyle::BlinkingBar
    )?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    let result = app(&mut terminal, store);
    disable_raw_mode()?;
    execute!(
        stdout(),
        event::DisableMouseCapture,
        LeaveAlternateScreen,
        crossterm::cursor::SetCursorStyle::DefaultUserShape
    )?;
    terminal.show_cursor()?;
    result
}

fn app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, store: &mut TodoStore) -> Result<()> {
    let mut filter = Filter::Pending;
    let mut selected = 0usize;
    let mut input = None::<(InputState, String, usize)>; // (模式, 内容, 光标字符索引)
    let mut confirm_delete = false;
    let mut list_state = ListState::default();
    let mut list_inner_rect = Rect::default();

    loop {
        if input.is_none() {
            let _ = store.reload();
        }
        let total_vis = visible_len(store, filter);
        if total_vis > 0 && selected >= total_vis {
            selected = total_vis - 1;
        } else if total_vis == 0 {
            selected = 0;
        }

        terminal.draw(|frame| {
            list_inner_rect = draw(
                frame,
                store,
                filter,
                selected,
                input.as_ref(),
                confirm_delete,
                &mut list_state,
            );
        })?;
        if !event::poll(std::time::Duration::from_millis(100))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if input.is_some() => {
                let (_mode, text, cursor) = input.as_mut().unwrap();
                match key.code {
                    KeyCode::Esc => input = None,
                    KeyCode::Enter => {
                        let (mode, title, _) = input.take().unwrap();
                        let title = title.trim().to_string();
                        if !title.is_empty() {
                            match mode {
                                InputState::Creating => {
                                    store.add(title)?;
                                }
                                InputState::Editing(idx) => {
                                    store.update_title(idx, title)?;
                                }
                            }
                        }
                    }
                    KeyCode::Left => {
                        *cursor = cursor.saturating_sub(1);
                    }
                    KeyCode::Right => {
                        let max_len = text.chars().count();
                        if *cursor < max_len {
                            *cursor += 1;
                        }
                    }
                    KeyCode::Home => {
                        *cursor = 0;
                    }
                    KeyCode::End => {
                        *cursor = text.chars().count();
                    }
                    KeyCode::Backspace => {
                        if *cursor > 0 {
                            let mut chars: Vec<char> = text.chars().collect();
                            let idx = (*cursor).min(chars.len());
                            chars.remove(idx - 1);
                            *text = chars.into_iter().collect();
                            *cursor -= 1;
                        }
                    }
                    KeyCode::Delete => {
                        let mut chars: Vec<char> = text.chars().collect();
                        if *cursor < chars.len() {
                            chars.remove(*cursor);
                            *text = chars.into_iter().collect();
                        }
                    }
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let mut chars: Vec<char> = text.chars().collect();
                        let idx = (*cursor).min(chars.len());
                        chars.insert(idx, c);
                        *text = chars.into_iter().collect();
                        *cursor += 1;
                    }
                    _ => {}
                }
            }
            Event::Key(key) => {
                if confirm_delete {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
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
                        code: KeyCode::Up | KeyCode::Char('k'),
                        ..
                    } => selected = selected.saturating_sub(1),
                    KeyEvent {
                        code: KeyCode::Down | KeyCode::Char('j'),
                        ..
                    } => {
                        selected = selected
                            .saturating_add(1)
                            .min(visible_len(store, filter).saturating_sub(1))
                    }
                    // 回车键 Enter 或 e 键：进入再次编辑模式
                    KeyEvent {
                        code: KeyCode::Enter | KeyCode::Char('e'),
                        ..
                    } => {
                        if let Some(index) = visible_index(store, filter, selected)
                            && let Some(item) = store.items().get(index)
                        {
                            let title = item.title.clone();
                            let cursor = title.chars().count();
                            input = Some((InputState::Editing(index), title, cursor));
                        }
                    }
                    // 空格键 Space：快速切换待办完成状态
                    KeyEvent {
                        code: KeyCode::Char(' '),
                        ..
                    } => {
                        if let Some(index) = visible_index(store, filter, selected) {
                            store.toggle(index)?;
                        }
                    }
                    KeyEvent {
                        code: KeyCode::Char('a') | KeyCode::Char('n'),
                        ..
                    } => input = Some((InputState::Creating, String::new(), 0)),
                    KeyEvent {
                        code: KeyCode::Char('d') | KeyCode::Delete,
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
                    && let Some((index, checkbox)) = item_at(
                        mouse.row,
                        mouse.column,
                        list_inner_rect,
                        list_state.offset(),
                        store,
                        filter,
                    )
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
    input: Option<&(InputState, String, usize)>,
    confirm: bool,
    list_state: &mut ListState,
) -> Rect {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // 顶部标题栏
            Constraint::Min(8),    // 主体双栏
            Constraint::Length(2), // 底部状态与快捷键提示
        ])
        .split(area);

    let pending = store.items().iter().filter(|item| !item.completed).count();
    let completed = store.items().len() - pending;

    // 1. 顶部 Header
    let header = Line::from(vec![
        Span::styled(
            " 📋 TODO ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" 极速终端待办清单"),
        Span::styled("   ⏳ 未完成: ", Style::default().fg(Color::Yellow)),
        Span::styled(
            pending.to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("   ✅ 已完成: ", Style::default().fg(Color::Green)),
        Span::styled(
            completed.to_string(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(header), chunks[0]);

    // 2. 主体左右分栏
    let visible: Vec<_> = store
        .items()
        .iter()
        .enumerate()
        .filter(|(_, item)| matches_filter(item.completed, filter))
        .collect();

    let list_title = if filter == Filter::Pending {
        format!(" ⏳ 未完成待办 ({}) ", visible.len())
    } else {
        format!(" ✅ 已完成归档 ({}) ", visible.len())
    };

    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(ui_idx, (_, item))| {
            let is_selected = ui_idx == selected.min(visible.len().saturating_sub(1));
            let prefix = if is_selected { "▶ " } else { "  " };

            let (check_icon, check_style) = if item.completed {
                ("✅ ", Style::default().fg(Color::Green))
            } else {
                ("⬜ ", Style::default().fg(Color::DarkGray))
            };

            let title_style = if item.completed {
                Style::default().fg(Color::DarkGray)
            } else if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(vec![
                Span::styled(
                    prefix,
                    if is_selected {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    },
                ),
                Span::styled(check_icon, check_style),
                Span::styled(&item.title, title_style),
            ]))
        })
        .collect();

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    let list_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            list_title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let list_inner = list_block.inner(body[0]);

    if items.is_empty() {
        let empty_tip = if filter == Filter::Pending {
            "🎉 暂无待办事项，按 [a] 新建一个吧！"
        } else {
            "📦 暂无已完成的待办记录"
        };
        let p = Paragraph::new(empty_tip)
            .block(list_block)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, body[0]);
    } else {
        list_state.select(Some(selected.min(visible.len().saturating_sub(1))));
        let list = List::new(items).block(list_block).highlight_style(
            Style::default()
                .bg(Color::Rgb(35, 45, 60))
                .add_modifier(Modifier::BOLD),
        );
        frame.render_stateful_widget(list, body[0], list_state);
    }

    // 3. 右侧详情预览
    let detail = visible.get(selected.min(visible.len().saturating_sub(1)));
    let detail_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " 🔍 详细信息 ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    if let Some((_, item)) = detail {
        let created_str = item.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
        let status_str = if item.completed {
            "已完成 ✅"
        } else {
            "进行中 ⏳"
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    "📌 待办: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    &item.title,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("🆔 编号: ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("#{}", item.id), Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled("🕒 创建: ", Style::default().fg(Color::DarkGray)),
                Span::styled(created_str, Style::default().fg(Color::Gray)),
            ]),
            Line::from(vec![
                Span::styled("🚦 状态: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    status_str,
                    if item.completed {
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    },
                ),
            ]),
        ];

        if let Some(time) = item.completed_at {
            lines.push(Line::from(vec![
                Span::styled("🎉 完成于: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    time.format("%Y-%m-%d %H:%M:%S").to_string(),
                    Style::default().fg(Color::Green),
                ),
            ]));
        }

        lines.push(Line::from(Span::styled(
            "─".repeat(body[1].width.saturating_sub(4) as usize),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("💡 提示: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "按 [Enter] 编辑内容，按 [Space] 切换完成状态",
                Style::default().fg(Color::Gray),
            ),
        ]));

        let p = Paragraph::new(lines)
            .block(detail_block)
            .wrap(Wrap { trim: false });
        frame.render_widget(p, body[1]);
    } else {
        let p = Paragraph::new("请选择左侧待办查看详情")
            .block(detail_block)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, body[1]);
    }

    // 4. 底部快捷键状态栏
    let footer_line = Line::from(vec![
        Span::styled(
            "[Enter]",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("编辑 "),
        Span::styled(
            "[Space]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("完成/取消 "),
        Span::styled(
            "[a]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("新建 "),
        Span::styled(
            "[Tab]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("切换待办/已完成 "),
        Span::styled(
            "[d]",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw("删除 "),
        Span::styled("[j/k/↑/↓/点击]", Style::default().fg(Color::Yellow)),
        Span::raw("选择 "),
        Span::styled("[q/Esc]", Style::default().fg(Color::DarkGray)),
        Span::raw("退出"),
    ]);
    frame.render_widget(Paragraph::new(footer_line), chunks[2]);

    // 5. 弹窗浮层
    if let Some((mode, value, cursor)) = input {
        draw_input(frame, mode, value, *cursor);
    }
    if confirm {
        let title = detail
            .map(|(_, item)| item.title.as_str())
            .unwrap_or("该待办");
        draw_confirm(frame, title);
    }

    list_inner
}

fn draw_input(frame: &mut Frame, mode: &InputState, value: &str, cursor: usize) {
    let area = centered(frame.area(), 65, 30);
    frame.render_widget(Clear, area);

    let modal_title = match mode {
        InputState::Creating => " ➕ 新增待办事项 ",
        InputState::Editing(_) => " ✏️ 编辑待办事项 ",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            modal_title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    frame.render_widget(block, area);

    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // 输入框
            Constraint::Length(1), // 底部操作提示
        ])
        .split(inner);

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow))
        .title(Span::styled(
            " 内容 (必填) ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    let input_inner = input_block.inner(chunks[0]);
    frame.render_widget(input_block, chunks[0]);
    let p_input = Paragraph::new(value);
    frame.render_widget(p_input, input_inner);

    // 硬件光标精确定位（IME / 中文输入法定位）
    let before_cursor: String = value.chars().take(cursor).collect();
    let text_w = UnicodeWidthStr::width(before_cursor.as_str()) as u16;
    let cursor_x = (input_inner.x + text_w).min(input_inner.right().saturating_sub(1));
    let cursor_y = input_inner.y;
    frame.set_cursor_position((cursor_x, cursor_y));

    let tip = Line::from(vec![
        Span::styled(
            "[Enter]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" 保存待办    "),
        Span::styled("[Esc]", Style::default().fg(Color::DarkGray)),
        Span::raw(" 取消"),
    ]);
    frame.render_widget(Paragraph::new(tip).alignment(Alignment::Center), chunks[1]);
}

fn draw_confirm(frame: &mut Frame, target_title: &str) {
    let area = centered(frame.area(), 50, 25);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Red))
        .title(Span::styled(
            " ⚠️ 删除确认 ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));

    let text = Text::from(vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("确定要删除待办「"),
            Span::styled(
                target_title,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("」吗？"),
        ]),
        Line::from(Span::styled(
            "此操作不可恢复！",
            Style::default().fg(Color::Red),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "[y / Enter] 确认删除",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled("[n / Esc] 取消", Style::default().fg(Color::Gray)),
        ]),
    ]);

    let p = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(p, area);
}

fn centered(area: Rect, width_pct: u16, height_pct: u16) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_pct) / 2),
            Constraint::Percentage(height_pct),
            Constraint::Percentage((100 - height_pct) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_pct) / 2),
            Constraint::Percentage(width_pct),
            Constraint::Percentage((100 - width_pct) / 2),
        ])
        .split(popup_layout[1])[1]
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

fn item_at(
    row: u16,
    column: u16,
    list_inner: Rect,
    offset: usize,
    store: &TodoStore,
    filter: Filter,
) -> Option<(usize, bool)> {
    if list_inner.width == 0 || list_inner.height == 0 {
        return None;
    }
    if row < list_inner.y || row >= list_inner.bottom() {
        return None;
    }
    if column < list_inner.x || column >= list_inner.right() {
        return None;
    }
    let line_offset = (row - list_inner.y) as usize;
    let visible_pos = offset + line_offset;
    let index = visible_index(store, filter, visible_pos)?;
    let rel_col = column.saturating_sub(list_inner.x);
    let is_checkbox = (2..=6).contains(&rel_col);
    Some((index, is_checkbox))
}
