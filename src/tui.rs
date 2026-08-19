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
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
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

#[derive(Clone, PartialEq, Eq)]
struct ActiveInput {
    mode: InputState,
    text: String,
    cursor: usize,
    scroll_top: usize,
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
    let mut input = None::<ActiveInput>;
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
                let size = terminal.size()?;
                let input_inner = input_box_inner(Rect::new(0, 0, size.width, size.height));
                let inner_w = input_inner.width as usize;
                let inner_h = input_inner.height as usize;

                let active = input.as_mut().unwrap();
                match key.code {
                    KeyCode::Esc => input = None,
                    KeyCode::Char('s') | KeyCode::Char('S')
                        if key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        let ActiveInput { mode, text, .. } = input.take().unwrap();
                        let title = text.trim().to_string();
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
                    KeyCode::Enter => {
                        let mut chars: Vec<char> = active.text.chars().collect();
                        let idx = active.cursor.min(chars.len());
                        chars.insert(idx, '\n');
                        active.text = chars.into_iter().collect();
                        active.cursor += 1;
                        ensure_cursor_visible(active, inner_w, inner_h);
                    }
                    KeyCode::Left => {
                        active.cursor = active.cursor.saturating_sub(1);
                        ensure_cursor_visible(active, inner_w, inner_h);
                    }
                    KeyCode::Right => {
                        let max_len = active.text.chars().count();
                        if active.cursor < max_len {
                            active.cursor += 1;
                        }
                        ensure_cursor_visible(active, inner_w, inner_h);
                    }
                    KeyCode::Up => {
                        let wrapped = wrap_input_text(&active.text, inner_w, active.cursor);
                        if wrapped.cursor_line > 0 {
                            active.cursor = char_at_line_col(
                                &active.text,
                                &wrapped,
                                wrapped.cursor_line - 1,
                                wrapped.cursor_col,
                            );
                            ensure_cursor_visible(active, inner_w, inner_h);
                        }
                    }
                    KeyCode::Down => {
                        let wrapped = wrap_input_text(&active.text, inner_w, active.cursor);
                        if wrapped.cursor_line + 1 < wrapped.lines.len() {
                            active.cursor = char_at_line_col(
                                &active.text,
                                &wrapped,
                                wrapped.cursor_line + 1,
                                wrapped.cursor_col,
                            );
                            ensure_cursor_visible(active, inner_w, inner_h);
                        }
                    }
                    KeyCode::PageUp => {
                        let page = inner_h.max(1);
                        active.scroll_top = active.scroll_top.saturating_sub(page);
                    }
                    KeyCode::PageDown => {
                        let page = inner_h.max(1);
                        let wrapped = wrap_input_text(&active.text, inner_w, active.cursor);
                        let max_scroll = wrapped.lines.len().saturating_sub(inner_h);
                        active.scroll_top = (active.scroll_top + page).min(max_scroll);
                    }
                    KeyCode::Home => {
                        active.cursor = 0;
                        ensure_cursor_visible(active, inner_w, inner_h);
                    }
                    KeyCode::End => {
                        active.cursor = active.text.chars().count();
                        ensure_cursor_visible(active, inner_w, inner_h);
                    }
                    KeyCode::Backspace => {
                        if active.cursor > 0 {
                            let mut chars: Vec<char> = active.text.chars().collect();
                            let idx = active.cursor.min(chars.len());
                            chars.remove(idx - 1);
                            active.text = chars.into_iter().collect();
                            active.cursor -= 1;
                            ensure_cursor_visible(active, inner_w, inner_h);
                        }
                    }
                    KeyCode::Delete => {
                        let mut chars: Vec<char> = active.text.chars().collect();
                        if active.cursor < chars.len() {
                            chars.remove(active.cursor);
                            active.text = chars.into_iter().collect();
                            ensure_cursor_visible(active, inner_w, inner_h);
                        }
                    }
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let mut chars: Vec<char> = active.text.chars().collect();
                        let idx = active.cursor.min(chars.len());
                        chars.insert(idx, c);
                        active.text = chars.into_iter().collect();
                        active.cursor += 1;
                        ensure_cursor_visible(active, inner_w, inner_h);
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
                    KeyEvent {
                        code: KeyCode::PageUp,
                        ..
                    } => selected = selected.saturating_sub(5),
                    KeyEvent {
                        code: KeyCode::PageDown,
                        ..
                    } => {
                        selected = selected
                            .saturating_add(5)
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
                            input = Some(ActiveInput {
                                mode: InputState::Editing(index),
                                text: title,
                                cursor,
                                scroll_top: 0,
                            });
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
                    } => {
                        input = Some(ActiveInput {
                            mode: InputState::Creating,
                            text: String::new(),
                            cursor: 0,
                            scroll_top: 0,
                        });
                    }
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
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => {
                    if let Some(active) = input.as_mut() {
                        active.scroll_top = active.scroll_top.saturating_sub(1);
                    } else {
                        selected = selected.saturating_sub(1);
                    }
                }
                MouseEventKind::ScrollDown => {
                    if let Some(active) = input.as_mut() {
                        let size = terminal.size()?;
                        let input_inner = input_box_inner(Rect::new(0, 0, size.width, size.height));
                        let wrapped = wrap_input_text(
                            &active.text,
                            input_inner.width as usize,
                            active.cursor,
                        );
                        let max_scroll = wrapped
                            .lines
                            .len()
                            .saturating_sub(input_inner.height as usize);
                        active.scroll_top = (active.scroll_top + 1).min(max_scroll);
                    } else {
                        selected = selected
                            .saturating_add(1)
                            .min(visible_len(store, filter).saturating_sub(1));
                    }
                }
                MouseEventKind::Down(MouseButton::Left) if input.is_none() => {
                    if let Some((index, checkbox)) = item_at(
                        mouse.row,
                        mouse.column,
                        list_inner_rect,
                        list_state.offset(),
                        store,
                        filter,
                    ) {
                        selected = visible_position(store, filter, index);
                        if checkbox {
                            store.toggle(index)?;
                        }
                    }
                }
                _ => {}
            },
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
    input: Option<&ActiveInput>,
    confirm: bool,
    list_state: &mut ListState,
) -> Rect {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // 顶部 Header 与分类切换提示
            Constraint::Min(6),    // 主体全宽待办列表
            Constraint::Length(4), // 底部紧凑详情面板
            Constraint::Length(1), // 底部快捷键指南
        ])
        .split(area);

    let pending = store.items().iter().filter(|item| !item.completed).count();
    let completed = store.items().len() - pending;

    // 1. 顶部 Header 与分类状态
    let pending_style = if filter == Filter::Pending {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let completed_style = if filter == Filter::Completed {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let header = Line::from(vec![
        Span::styled(
            " 📋 TODO ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(format!(" [Tab] ⏳ 进行中 ({pending}) "), pending_style),
        Span::raw(" "),
        Span::styled(format!(" ✅ 已完成 ({completed}) "), completed_style),
        Span::raw("  "),
        Span::styled("| 按 [Tab] 切换分类", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(header), chunks[0]);

    // 2. 主体全宽待办列表
    let visible: Vec<_> = store
        .items()
        .iter()
        .enumerate()
        .filter(|(_, item)| matches_filter(item.completed, filter))
        .collect();

    let list_title = if filter == Filter::Pending {
        format!(" ⏳ 待办事项列表 ({}) ", visible.len())
    } else {
        format!(" ✅ 已完成归档列表 ({}) ", visible.len())
    };

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

    let list_inner = list_block.inner(chunks[1]);
    let total_width = list_inner.width as usize;

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

            let num_str = format!("{}. ", ui_idx + 1);

            let meta_str = if let Some(time) = item.completed_at {
                format!("🎉 {}  #{}", time.format("%m-%d %H:%M"), item.id)
            } else {
                format!("🕒 {}  #{}", item.created_at.format("%m-%d %H:%M"), item.id)
            };

            let prefix_w = 2;
            let check_w = 3;
            let num_w = UnicodeWidthStr::width(num_str.as_str());
            let meta_w = UnicodeWidthStr::width(meta_str.as_str());

            let fixed_w = prefix_w + check_w + num_w + meta_w + 2;
            let avail_title_w = total_width.saturating_sub(fixed_w);

            let display_source = item.title.replace('\n', " ↵ ");
            let (display_title, title_w) = truncate_to_width(&display_source, avail_title_w);
            let spacing = total_width.saturating_sub(prefix_w + check_w + num_w + title_w + meta_w);

            let title_style = if item.completed {
                Style::default().fg(Color::DarkGray)
            } else if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let line = Line::from(vec![
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
                Span::styled(num_str, Style::default().fg(Color::DarkGray)),
                Span::styled(display_title, title_style),
                Span::raw(" ".repeat(spacing)),
                Span::styled(
                    meta_str,
                    if is_selected {
                        Style::default().fg(Color::Gray)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    if items.is_empty() {
        let empty_tip = if filter == Filter::Pending {
            "🎉 暂无未完成待办事项，按 [a] 或 [n] 快速新建一个吧！"
        } else {
            "📦 暂无已完成的待办归档记录"
        };
        let p = Paragraph::new(empty_tip)
            .block(list_block)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, chunks[1]);
    } else {
        list_state.select(Some(selected.min(visible.len().saturating_sub(1))));
        let list = List::new(items).block(list_block).highlight_style(
            Style::default()
                .bg(Color::Rgb(35, 45, 60))
                .add_modifier(Modifier::BOLD),
        );
        frame.render_stateful_widget(list, chunks[1], list_state);
    }

    // 3. 底部紧凑详情卡片
    let detail = visible.get(selected.min(visible.len().saturating_sub(1)));
    let detail_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " 📌 选中待办详情 ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    if let Some((_, item)) = detail {
        let created_str = item.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
        let status_span = if item.completed {
            Span::styled(
                "已完成 ✅",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                "进行中 ⏳",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        };

        let mut meta_spans = vec![
            Span::styled("🆔 编号: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("#{}   ", item.id), Style::default().fg(Color::Cyan)),
            Span::styled("🕒 创建于: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{created_str}   "),
                Style::default().fg(Color::Gray),
            ),
            Span::styled("🚦 状态: ", Style::default().fg(Color::DarkGray)),
            status_span,
        ];

        if let Some(time) = item.completed_at {
            meta_spans.push(Span::styled(
                "   🎉 完成于: ",
                Style::default().fg(Color::DarkGray),
            ));
            meta_spans.push(Span::styled(
                time.format("%Y-%m-%d %H:%M:%S").to_string(),
                Style::default().fg(Color::Green),
            ));
        }

        let display_title = item.title.replace('\n', " ↵ ");
        let lines = vec![
            Line::from(vec![
                Span::styled(
                    "📝 内容: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    display_title,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(meta_spans),
        ];

        let p = Paragraph::new(lines).block(detail_block);
        frame.render_widget(p, chunks[2]);
    } else {
        let p = Paragraph::new("💡 当前分类暂无待办事项，按 [a] 或 [n] 快速新建一条吧！")
            .block(detail_block)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, chunks[2]);
    }

    // 4. 底部快捷键状态栏
    let footer_line = Line::from(vec![
        Span::styled(
            "[a/n]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("新建 "),
        Span::styled(
            "[Enter/e]",
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
        Span::raw("切换完成 "),
        Span::styled(
            "[Tab]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("分类切换 "),
        Span::styled(
            "[d]",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw("删除 "),
        Span::styled("[j/k/↑/↓/滚轮]", Style::default().fg(Color::Yellow)),
        Span::raw("移动选择 "),
        Span::styled("[q/Esc]", Style::default().fg(Color::DarkGray)),
        Span::raw("退出"),
    ]);
    frame.render_widget(Paragraph::new(footer_line), chunks[3]);

    // 5. 弹窗浮层
    if let Some(active) = input {
        draw_input(frame, active);
    }
    if confirm {
        let title = detail
            .map(|(_, item)| item.title.as_str())
            .unwrap_or("该待办");
        draw_confirm(frame, title);
    }

    list_inner
}

fn input_box_inner(area: Rect) -> Rect {
    let modal_area = centered(area, 70, 45);
    let inner = Rect {
        x: modal_area.x + 2,
        y: modal_area.y + 1,
        width: modal_area.width.saturating_sub(4),
        height: modal_area.height.saturating_sub(2),
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // 输入框，占满可用空间
            Constraint::Length(1), // 底部操作提示
        ])
        .split(inner);
    let input_block = Block::default().borders(Borders::ALL);
    input_block.inner(chunks[0])
}

fn ensure_cursor_visible(input: &mut ActiveInput, width: usize, height: usize) {
    let height = height.max(1);
    let wrapped = wrap_input_text(&input.text, width, input.cursor);
    if wrapped.cursor_line < input.scroll_top {
        input.scroll_top = wrapped.cursor_line;
    } else if wrapped.cursor_line >= input.scroll_top + height {
        input.scroll_top = wrapped.cursor_line + 1 - height;
    }
}

#[derive(Debug, PartialEq, Eq)]
struct WrappedInput {
    lines: Vec<String>,
    cursor_line: usize,
    cursor_col: usize,
    line_char_ranges: Vec<std::ops::Range<usize>>,
}

fn wrap_input_text(text: &str, max_width: usize, cursor_char_idx: usize) -> WrappedInput {
    let max_width = max_width.max(1);
    let mut lines = Vec::new();
    let mut line_char_ranges = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;
    let mut line_start_char = 0;

    let mut cursor_pos = None;
    let chars: Vec<char> = text.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        if i == cursor_char_idx {
            cursor_pos = Some((lines.len(), current_width));
        }

        if c == '\n' {
            line_char_ranges.push(line_start_char..i + 1);
            lines.push(current_line);
            current_line = String::new();
            current_width = 0;
            line_start_char = i + 1;
            continue;
        }

        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(1);

        if current_width + cw > max_width && current_width > 0 {
            line_char_ranges.push(line_start_char..i);
            lines.push(current_line);
            current_line = String::new();
            current_width = 0;
            line_start_char = i;
        }

        current_line.push(c);
        current_width += cw;
    }

    if cursor_pos.is_none() && cursor_char_idx >= chars.len() {
        cursor_pos = Some((lines.len(), current_width));
    }

    line_char_ranges.push(line_start_char..chars.len());
    lines.push(current_line);

    let (cursor_line, cursor_col) = cursor_pos.unwrap_or((0, 0));
    WrappedInput {
        lines,
        cursor_line,
        cursor_col,
        line_char_ranges,
    }
}

fn char_at_line_col(
    text: &str,
    wrapped: &WrappedInput,
    target_line: usize,
    target_col: usize,
) -> usize {
    if target_line >= wrapped.line_char_ranges.len() {
        return text.chars().count();
    }
    let range = &wrapped.line_char_ranges[target_line];
    let chars: Vec<char> = text.chars().collect();
    let start = range.start;
    let end = range.end.min(chars.len());
    let mut w = 0;
    for (offset, &c) in chars[start..end].iter().enumerate() {
        if c == '\n' {
            return start + offset;
        }
        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(1);
        if w + cw > target_col {
            return start + offset;
        }
        w += cw;
    }
    if end > start && end <= chars.len() && chars[end - 1] == '\n' {
        return end - 1;
    }
    end
}

fn draw_input(frame: &mut Frame, active: &ActiveInput) {
    let modal_area = centered(frame.area(), 70, 45);
    frame.render_widget(Clear, modal_area);

    let modal_title = match active.mode {
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
    frame.render_widget(block, modal_area);

    let inner = Rect {
        x: modal_area.x + 2,
        y: modal_area.y + 1,
        width: modal_area.width.saturating_sub(4),
        height: modal_area.height.saturating_sub(2),
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // 输入框，自适应剩余高度
            Constraint::Length(1), // 底部操作提示
        ])
        .split(inner);

    let max_w = chunks[0].width.saturating_sub(2) as usize;
    let max_h = chunks[0].height.saturating_sub(2) as usize;
    let wrapped = wrap_input_text(&active.text, max_w, active.cursor);

    let max_scroll = wrapped.lines.len().saturating_sub(max_h);
    let scroll_y = active.scroll_top.min(max_scroll);

    let scroll_info = if wrapped.lines.len() > max_h {
        format!(
            " (第 {}/{} 行，滚轮/PgUp/PgDn翻页) ",
            scroll_y + 1,
            wrapped.lines.len()
        )
    } else {
        String::new()
    };

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow))
        .title(Span::styled(
            format!(" 内容 (必填){scroll_info}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    let input_inner = input_block.inner(chunks[0]);
    frame.render_widget(input_block, chunks[0]);

    let visible_lines: Vec<Line> = wrapped
        .lines
        .iter()
        .skip(scroll_y)
        .take(max_h)
        .map(|l| Line::from(l.as_str()))
        .collect();

    let p_input = Paragraph::new(visible_lines);
    frame.render_widget(p_input, input_inner);

    // 硬件光标定位（若光标在当前滚动可视区域内，精准映射到终端坐标与 IME）
    if wrapped.cursor_line >= scroll_y && wrapped.cursor_line < scroll_y + max_h {
        let cursor_rel_line = (wrapped.cursor_line - scroll_y) as u16;
        let cursor_x =
            (input_inner.x + wrapped.cursor_col as u16).min(input_inner.right().saturating_sub(1));
        let cursor_y =
            (input_inner.y + cursor_rel_line).min(input_inner.bottom().saturating_sub(1));
        frame.set_cursor_position((cursor_x, cursor_y));
    }

    let tip = Line::from(vec![
        Span::styled(
            "[Ctrl+S]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" 保存    "),
        Span::styled(
            "[Enter]",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" 换行    "),
        Span::styled(
            "[↑/↓/←/→]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" 光标    "),
        Span::styled(
            "[滚轮/PgUp/PgDn]",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" 翻页    "),
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
    // 列表项前缀为 "▶ ⬜ 1. "，0..=7 列点击直接切换完成状态
    let is_checkbox = rel_col <= 7;
    Some((index, is_checkbox))
}

fn truncate_to_width(s: &str, max_width: usize) -> (String, usize) {
    let total_str_width = UnicodeWidthStr::width(s);
    if total_str_width <= max_width {
        return (s.to_string(), total_str_width);
    }
    if max_width <= 3 {
        return ("...".to_string(), 3);
    }
    let target_width = max_width - 3;
    let mut current_width = 0;
    let mut result = String::new();
    for c in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if current_width + cw > target_width {
            break;
        }
        result.push(c);
        current_width += cw;
    }
    result.push_str("...");
    (result, current_width + 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_input_empty() {
        let wrapped = wrap_input_text("", 10, 0);
        assert_eq!(wrapped.lines, vec![""]);
        assert_eq!(wrapped.cursor_line, 0);
        assert_eq!(wrapped.cursor_col, 0);
    }

    #[test]
    fn test_wrap_input_single_line() {
        let wrapped = wrap_input_text("hello", 10, 3);
        assert_eq!(wrapped.lines, vec!["hello"]);
        assert_eq!(wrapped.cursor_line, 0);
        assert_eq!(wrapped.cursor_col, 3);
    }

    #[test]
    fn test_wrap_input_multi_line_ascii() {
        let text = "abcdefghij";
        let wrapped = wrap_input_text(text, 4, 6);
        assert_eq!(wrapped.lines, vec!["abcd", "efgh", "ij"]);
        assert_eq!(wrapped.cursor_line, 1);
        assert_eq!(wrapped.cursor_col, 2); // 'g' is at col 2 on line 1
    }

    #[test]
    fn test_wrap_input_cjk_boundary() {
        let text = "待办事项测试多行显示功能";
        // Each Chinese character width is 2. Width 6 holds 3 chars ("待办事", "项测试", "多行显", "示功能")
        let wrapped = wrap_input_text(text, 6, 4); // char 4 is '测'
        assert_eq!(wrapped.lines, vec!["待办事", "项测试", "多行显", "示功能"]);
        assert_eq!(wrapped.cursor_line, 1);
        assert_eq!(wrapped.cursor_col, 2); // '测' is at col 2 on line 1 ("项"=2, '测' starts at 2)
    }

    #[test]
    fn test_wrap_input_with_newlines() {
        let text = "第一行\n第二行内容\n\n第四行";
        let wrapped = wrap_input_text(text, 20, 4); // cursor at '第' of 第二行
        assert_eq!(wrapped.lines, vec!["第一行", "第二行内容", "", "第四行"]);
        assert_eq!(wrapped.cursor_line, 1);
        assert_eq!(wrapped.cursor_col, 0);
    }

    #[test]
    fn test_ensure_cursor_visible_scrolling() {
        let mut input = ActiveInput {
            mode: InputState::Creating,
            text: "1\n2\n3\n4\n5\n6\n7".to_string(),
            cursor: 12, // at '7' (line 6)
            scroll_top: 0,
        };
        // Box height is 3 lines
        ensure_cursor_visible(&mut input, 20, 3);
        // Should scroll down so line 6 is visible (scroll_top = 4: lines 4, 5, 6 visible)
        assert_eq!(input.scroll_top, 4);

        // Move cursor back to line 0 ('1')
        input.cursor = 0;
        ensure_cursor_visible(&mut input, 20, 3);
        assert_eq!(input.scroll_top, 0);
    }

    #[test]
    fn test_char_at_line_col_navigation() {
        let text = "abcdefghij";
        let wrapped = wrap_input_text(text, 4, 6);
        let up_idx = char_at_line_col(text, &wrapped, 0, 2);
        assert_eq!(up_idx, 2);
        let down_idx = char_at_line_col(text, &wrapped, 2, 2);
        assert_eq!(down_idx, 10);
    }
}
