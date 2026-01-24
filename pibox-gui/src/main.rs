//! pibox-gui: Graphical UI client for pibox using iced
//!
//! Features:
//! - Same vim-style keybindings as TUI
//! - Thumbnail preview
//! - Adaptive rendering (GPU when available)

use iced::{
    keyboard::{self, Key},
    widget::{column, container, row, scrollable, text, Column},
    Alignment, Color, Element, Length, Subscription, Task, Theme,
};
use pibox_core::{
    state::{FileEntry, FileType, InputMode, StatusLevel},
    Config,
};

fn main() -> iced::Result {
    iced::application("pibox", PiboxGui::update, PiboxGui::view)
        .subscription(PiboxGui::subscription)
        .theme(PiboxGui::theme)
        .window_size((1200.0, 800.0))
        .run_with(PiboxGui::new)
}

/// Main application state
#[derive(Default)]
struct PiboxGui {
    entries: Vec<FileEntry>,
    cursor: usize,
    selected: Vec<usize>,
    current_path: String,
    status_message: Option<(String, StatusLevel)>,
    input_mode: InputMode,
    search_query: String,
    connected: bool,
}

/// Application messages
#[derive(Debug, Clone)]
enum Message {
    // Navigation
    CursorUp,
    CursorDown,
    CursorTop,
    CursorBottom,
    Enter,
    Back,

    // Selection
    ToggleSelect,
    SelectAll,
    ClearSelection,

    // Actions
    Delete,
    Copy,
    Paste,
    Rename,

    // Mode
    EnterSearch,
    ExitMode,

    // Misc
    KeyPressed(keyboard::Key, keyboard::Modifiers),
}

impl PiboxGui {
    fn new() -> (Self, Task<Message>) {
        let mut app = Self {
            current_path: "/".to_string(),
            ..Default::default()
        };

        // Load demo data
        app.load_demo_data();

        (app, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CursorUp => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            Message::CursorDown => {
                if self.cursor < self.entries.len().saturating_sub(1) {
                    self.cursor += 1;
                }
            }
            Message::CursorTop => {
                self.cursor = 0;
            }
            Message::CursorBottom => {
                self.cursor = self.entries.len().saturating_sub(1);
            }
            Message::Enter => {
                if let Some(entry) = self.entries.get(self.cursor) {
                    if entry.is_dir() {
                        self.set_status(format!("Navigate to: {}", entry.path), StatusLevel::Info);
                    } else {
                        self.set_status(format!("Open: {}", entry.name), StatusLevel::Info);
                    }
                }
            }
            Message::Back => {
                if self.current_path != "/" {
                    self.set_status("Navigate up", StatusLevel::Info);
                }
            }
            Message::ToggleSelect => {
                if self.selected.contains(&self.cursor) {
                    self.selected.retain(|&i| i != self.cursor);
                } else {
                    self.selected.push(self.cursor);
                }
            }
            Message::SelectAll => {
                self.selected = (0..self.entries.len()).collect();
            }
            Message::ClearSelection => {
                self.selected.clear();
            }
            Message::Delete => {
                let count = if self.selected.is_empty() { 1 } else { self.selected.len() };
                self.set_status(format!("Delete {} item(s)?", count), StatusLevel::Warning);
            }
            Message::Copy => {
                let count = if self.selected.is_empty() { 1 } else { self.selected.len() };
                self.set_status(format!("Copied {} item(s)", count), StatusLevel::Success);
            }
            Message::Paste => {
                self.set_status("Paste (not implemented)", StatusLevel::Info);
            }
            Message::Rename => {
                self.input_mode = InputMode::Rename;
            }
            Message::EnterSearch => {
                self.input_mode = InputMode::Search;
                self.search_query.clear();
            }
            Message::ExitMode => {
                self.input_mode = InputMode::Normal;
                self.search_query.clear();
            }
            Message::KeyPressed(key, modifiers) => {
                return self.handle_key(key, modifiers);
            }
        }

        Task::none()
    }

    fn view(&self) -> Element<Message> {
        // Main layout: file list + status bar
        let content = column![
            self.view_toolbar(),
            self.view_file_list(),
            self.view_status_bar(),
        ]
        .spacing(0);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        keyboard::on_key_press(|key, modifiers| Some(Message::KeyPressed(key, modifiers)))
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn load_demo_data(&mut self) {
        self.entries = vec![
            FileEntry {
                name: "Documents".to_string(),
                path: "/Documents".to_string(),
                file_type: FileType::Directory,
                size: 0,
                modified: 1706000000,
                mime_type: None,
            },
            FileEntry {
                name: "Downloads".to_string(),
                path: "/Downloads".to_string(),
                file_type: FileType::Directory,
                size: 0,
                modified: 1706100000,
                mime_type: None,
            },
            FileEntry {
                name: "Movies".to_string(),
                path: "/Movies".to_string(),
                file_type: FileType::Directory,
                size: 0,
                modified: 1705900000,
                mime_type: None,
            },
            FileEntry {
                name: "Music".to_string(),
                path: "/Music".to_string(),
                file_type: FileType::Directory,
                size: 0,
                modified: 1705800000,
                mime_type: None,
            },
            FileEntry {
                name: "readme.txt".to_string(),
                path: "/readme.txt".to_string(),
                file_type: FileType::File,
                size: 1234,
                modified: 1706200000,
                mime_type: Some("text/plain".to_string()),
            },
            FileEntry {
                name: "photo.jpg".to_string(),
                path: "/photo.jpg".to_string(),
                file_type: FileType::File,
                size: 2_500_000,
                modified: 1706150000,
                mime_type: Some("image/jpeg".to_string()),
            },
            FileEntry {
                name: "video.mp4".to_string(),
                path: "/video.mp4".to_string(),
                file_type: FileType::File,
                size: 150_000_000,
                modified: 1706050000,
                mime_type: Some("video/mp4".to_string()),
            },
        ];
        self.set_status("Demo mode (no server connection)", StatusLevel::Info);
    }

    fn set_status(&mut self, message: impl Into<String>, level: StatusLevel) {
        self.status_message = Some((message.into(), level));
    }

    fn handle_key(&mut self, key: Key, modifiers: keyboard::Modifiers) -> Task<Message> {
        if self.input_mode != InputMode::Normal {
            // In input mode, only handle Escape
            if matches!(key, Key::Named(keyboard::key::Named::Escape)) {
                return Task::done(Message::ExitMode);
            }
            return Task::none();
        }

        // Vim-style keybindings
        match key {
            Key::Character(ref c) => {
                let c = c.as_str();
                match c {
                    "j" => return Task::done(Message::CursorDown),
                    "k" => return Task::done(Message::CursorUp),
                    "h" => return Task::done(Message::Back),
                    "l" => return Task::done(Message::Enter),
                    "g" => return Task::done(Message::CursorTop),
                    "G" => return Task::done(Message::CursorBottom),
                    " " => return Task::done(Message::ToggleSelect),
                    "d" => return Task::done(Message::Delete),
                    "y" => return Task::done(Message::Copy),
                    "p" => return Task::done(Message::Paste),
                    "r" => return Task::done(Message::Rename),
                    "/" => return Task::done(Message::EnterSearch),
                    "a" if modifiers.control() => return Task::done(Message::SelectAll),
                    _ => {}
                }
            }
            Key::Named(named) => match named {
                keyboard::key::Named::ArrowUp => return Task::done(Message::CursorUp),
                keyboard::key::Named::ArrowDown => return Task::done(Message::CursorDown),
                keyboard::key::Named::ArrowLeft => return Task::done(Message::Back),
                keyboard::key::Named::ArrowRight | keyboard::key::Named::Enter => {
                    return Task::done(Message::Enter)
                }
                keyboard::key::Named::Escape => return Task::done(Message::ClearSelection),
                keyboard::key::Named::Home => return Task::done(Message::CursorTop),
                keyboard::key::Named::End => return Task::done(Message::CursorBottom),
                _ => {}
            },
            _ => {}
        }

        Task::none()
    }

    fn view_toolbar(&self) -> Element<Message> {
        let path_text = text(&self.current_path).size(16);
        let connection_status = if self.connected {
            text("Connected").color(Color::from_rgb(0.2, 0.8, 0.2))
        } else {
            text("Offline").color(Color::from_rgb(0.8, 0.8, 0.2))
        };

        container(
            row![path_text, iced::widget::horizontal_space(), connection_status]
                .spacing(10)
                .padding(8),
        )
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(Color::from_rgb(0.15, 0.15, 0.15))),
            ..Default::default()
        })
        .width(Length::Fill)
        .into()
    }

    fn view_file_list(&self) -> Element<Message> {
        let items: Vec<Element<Message>> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, entry)| self.view_file_entry(i, entry))
            .collect();

        let list = Column::with_children(items).spacing(0);

        scrollable(container(list).width(Length::Fill).padding(4))
            .height(Length::Fill)
            .into()
    }

    fn view_file_entry(&self, index: usize, entry: &FileEntry) -> Element<Message> {
        let is_cursor = index == self.cursor;
        let is_selected = self.selected.contains(&index);

        // Icon
        let icon = match entry.file_type {
            FileType::Directory => "D ",
            FileType::File => match entry.mime_type.as_deref() {
                Some(mime) if mime.starts_with("image/") => "I ",
                Some(mime) if mime.starts_with("video/") => "V ",
                Some(mime) if mime.starts_with("audio/") => "A ",
                _ => "F ",
            },
            FileType::Symlink => "L ",
        };

        // Selection marker
        let marker = if is_selected { "* " } else { "  " };

        // Size - clone to own the data
        let size_str = if entry.is_dir() {
            String::new()
        } else {
            format_size(entry.size)
        };

        // Clone the name to avoid lifetime issues
        let name = entry.name.clone();

        let row_content = row![
            text(marker).width(Length::Fixed(20.0)),
            text(icon).width(Length::Fixed(30.0)),
            text(name).width(Length::FillPortion(3)),
            text(size_str).width(Length::Fixed(100.0)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let bg_color = if is_cursor {
            Color::from_rgb(0.2, 0.4, 0.6)
        } else if is_selected {
            Color::from_rgb(0.25, 0.25, 0.3)
        } else {
            Color::TRANSPARENT
        };

        container(row_content)
            .style(move |_theme| container::Style {
                background: Some(iced::Background::Color(bg_color)),
                ..Default::default()
            })
            .width(Length::Fill)
            .padding(6)
            .into()
    }

    fn view_status_bar(&self) -> Element<Message> {
        let status_text = if let Some((ref msg, ref level)) = self.status_message {
            let color = match level {
                StatusLevel::Info => Color::from_rgb(0.4, 0.6, 0.9),
                StatusLevel::Success => Color::from_rgb(0.3, 0.8, 0.3),
                StatusLevel::Warning => Color::from_rgb(0.9, 0.8, 0.2),
                StatusLevel::Error => Color::from_rgb(0.9, 0.3, 0.3),
            };
            text(msg).color(color)
        } else {
            text("j/k:move h/l:nav space:select d:del y:copy p:paste /:search")
                .color(Color::from_rgb(0.5, 0.5, 0.5))
        };

        container(status_text)
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(Color::from_rgb(0.12, 0.12, 0.12))),
                ..Default::default()
            })
            .width(Length::Fill)
            .padding(6)
            .into()
    }
}

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
