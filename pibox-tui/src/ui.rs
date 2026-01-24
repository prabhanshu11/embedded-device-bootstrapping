//! Terminal UI rendering with ratatui

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use pibox_core::state::{FileType, InputMode, StatusLevel};

use crate::app::App;

/// Main draw function
pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title bar
            Constraint::Min(1),    // File list
            Constraint::Length(1), // Status bar
            Constraint::Length(1), // Input line (for search/command)
        ])
        .split(f.area());

    draw_title_bar(f, app, chunks[0]);
    draw_file_list(f, app, chunks[1]);
    draw_status_bar(f, app, chunks[2]);
    draw_input_line(f, app, chunks[3]);
}

/// Draw the title bar with current path
fn draw_title_bar(f: &mut Frame, app: &App, area: Rect) {
    let title = format!(" {} ", app.state.current_path);
    let connected_indicator = if app.connected { " [Connected]" } else { " [Offline]" };

    let title_bar = Paragraph::new(Line::from(vec![
        Span::styled(title, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(
            connected_indicator,
            Style::default().fg(if app.connected { Color::Green } else { Color::Yellow }),
        ),
    ]))
    .style(Style::default().bg(Color::DarkGray));

    f.render_widget(title_bar, area);
}

/// Draw the file list
fn draw_file_list(f: &mut Frame, app: &App, area: Rect) {
    let visible_height = area.height as usize;
    let start = app.state.scroll_offset;
    let end = (start + visible_height).min(app.state.entries.len());

    let items: Vec<ListItem> = app
        .state
        .entries
        .iter()
        .enumerate()
        .skip(start)
        .take(visible_height)
        .map(|(i, entry)| {
            let is_selected = app.state.selected.contains(&i);
            let is_cursor = i == app.state.cursor;

            // Icon based on file type
            let icon = match entry.file_type {
                FileType::Directory => "📁 ",
                FileType::File => match entry.mime_type.as_deref() {
                    Some(mime) if mime.starts_with("image/") => "🖼  ",
                    Some(mime) if mime.starts_with("video/") => "🎬 ",
                    Some(mime) if mime.starts_with("audio/") => "🎵 ",
                    Some(mime) if mime.starts_with("text/") => "📄 ",
                    _ => "📄 ",
                },
                FileType::Symlink => "🔗 ",
            };

            // Format size
            let size_str = if entry.is_dir() {
                String::new()
            } else {
                format_size(entry.size)
            };

            // Selection marker
            let marker = if is_selected { "* " } else { "  " };

            // Build the line
            let line = format!("{}{}{:<40} {:>10}", marker, icon, entry.name, size_str);

            let style = if is_cursor {
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::Yellow)
            } else {
                Style::default()
            };

            ListItem::new(Line::from(Span::styled(line, style)))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::NONE),
    );

    f.render_widget(list, area);
}

/// Draw the status bar
fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let (text, style) = if let Some((ref msg, ref level)) = app.state.status_message {
        let color = match level {
            StatusLevel::Info => Color::Blue,
            StatusLevel::Success => Color::Green,
            StatusLevel::Warning => Color::Yellow,
            StatusLevel::Error => Color::Red,
        };
        (msg.clone(), Style::default().fg(color))
    } else {
        // Default hints based on mode
        let hints = match app.state.input_mode {
            InputMode::Normal => "j↓ k↑ l→ h← │ Space:select │ d:delete y:copy p:paste │ /:search ?:help q:quit",
            InputMode::Search => "Type to search │ Enter:confirm │ Esc:cancel",
            InputMode::Command => "Type command │ Enter:execute │ Esc:cancel",
            InputMode::Rename => "Enter new name │ Enter:confirm │ Esc:cancel",
            InputMode::Confirm(_) => "y:yes n:no │ Enter:confirm │ Esc:cancel",
        };
        (hints.to_string(), Style::default().fg(Color::DarkGray))
    };

    let status_bar = Paragraph::new(text).style(style);
    f.render_widget(status_bar, area);
}

/// Draw the input line (for search/command modes)
fn draw_input_line(f: &mut Frame, app: &App, area: Rect) {
    let (prefix, content) = match app.state.input_mode {
        InputMode::Search => ("/", &app.state.search_query),
        InputMode::Command => (":", &app.state.command_input),
        InputMode::Rename => ("Rename: ", &app.state.command_input),
        _ => ("", &String::new()),
    };

    let input_line = Paragraph::new(format!("{}{}", prefix, content))
        .style(Style::default().fg(Color::White));

    f.render_widget(input_line, area);

    // Show cursor in input modes
    if matches!(
        app.state.input_mode,
        InputMode::Search | InputMode::Command | InputMode::Rename
    ) {
        let x = area.x + prefix.len() as u16 + content.len() as u16;
        f.set_cursor_position((x, area.y));
    }
}

/// Format file size in human-readable form
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
