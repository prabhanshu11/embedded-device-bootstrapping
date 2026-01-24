//! Application state machine for pibox clients
//!
//! Implements an Elm-style architecture:
//! - Immutable state updates via messages
//! - Undo/redo support via command history
//! - Virtual file tree for memory efficiency

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Maximum undo history size (to bound memory usage)
const MAX_UNDO_HISTORY: usize = 50;

/// File type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    Directory,
    File,
    Symlink,
}

/// A file or directory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub file_type: FileType,
    pub size: u64,
    pub modified: i64,
    pub mime_type: Option<String>,
}

impl FileEntry {
    pub fn is_dir(&self) -> bool {
        matches!(self.file_type, FileType::Directory)
    }
}

/// Selection mode for multi-select operations
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SelectionMode {
    #[default]
    Single,
    Multi,
    Range,
}

/// Current view mode
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ViewMode {
    #[default]
    List,
    Grid,
    Tree,
}

/// UI input mode (vim-style)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum InputMode {
    #[default]
    Normal,
    /// Typing in search bar
    Search,
    /// Typing command (: prefix)
    Command,
    /// Rename dialog
    Rename,
    /// Confirmation prompt
    Confirm(ConfirmAction),
}

/// Actions requiring confirmation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmAction {
    Delete(Vec<String>),
    Overwrite(String),
}

/// Connection state to server
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Reconnecting { attempt: u32 },
}

/// Application state
#[derive(Debug, Clone)]
pub struct AppState {
    // Navigation
    pub current_path: String,
    pub entries: Vec<FileEntry>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub visible_rows: usize,

    // Selection
    pub selection_mode: SelectionMode,
    pub selected: Vec<usize>,
    pub selection_anchor: Option<usize>,

    // UI state
    pub view_mode: ViewMode,
    pub input_mode: InputMode,
    pub search_query: String,
    pub command_input: String,
    pub status_message: Option<(String, StatusLevel)>,

    // Connection
    pub connection: ConnectionState,
    pub server_url: String,

    // Pending operations
    pub pending_ops: Vec<PendingOp>,

    // Undo/redo
    undo_stack: VecDeque<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
}

/// Status message severity
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// Pending async operation
#[derive(Debug, Clone)]
pub struct PendingOp {
    pub id: String,
    pub op_type: OpType,
    pub progress: Option<f32>,
}

#[derive(Debug, Clone)]
pub enum OpType {
    Upload { path: String, size: u64 },
    Download { path: String, size: u64 },
    Delete { paths: Vec<String> },
    Rename { from: String, to: String },
}

/// Entry in undo history
#[derive(Debug, Clone)]
struct UndoEntry {
    action: UndoAction,
    description: String,
}

#[derive(Debug, Clone)]
enum UndoAction {
    Navigate { from: String },
    CursorMove { from: usize },
    Selection { previous: Vec<usize> },
    // File ops are not undoable (would require server-side support)
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_path: "/".to_string(),
            entries: Vec::new(),
            cursor: 0,
            scroll_offset: 0,
            visible_rows: 20,

            selection_mode: SelectionMode::Single,
            selected: Vec::new(),
            selection_anchor: None,

            view_mode: ViewMode::List,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            command_input: String::new(),
            status_message: None,

            connection: ConnectionState::Disconnected,
            server_url: String::new(),

            pending_ops: Vec::new(),

            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
        }
    }
}

impl AppState {
    pub fn new(server_url: &str) -> Self {
        Self {
            server_url: server_url.to_string(),
            ..Default::default()
        }
    }

    /// Update directory listing
    pub fn set_entries(&mut self, path: String, entries: Vec<FileEntry>) {
        self.push_undo(UndoAction::Navigate {
            from: self.current_path.clone(),
        }, "navigate");

        self.current_path = path;
        self.entries = entries;
        self.cursor = 0;
        self.scroll_offset = 0;
        self.selected.clear();
        self.selection_anchor = None;
    }

    /// Move cursor down
    pub fn cursor_down(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.push_undo(UndoAction::CursorMove { from: self.cursor }, "cursor");

        if self.cursor < self.entries.len() - 1 {
            self.cursor += 1;
            self.ensure_cursor_visible();
        }

        if self.selection_mode == SelectionMode::Range {
            self.update_range_selection();
        }
    }

    /// Move cursor up
    pub fn cursor_up(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.push_undo(UndoAction::CursorMove { from: self.cursor }, "cursor");

        if self.cursor > 0 {
            self.cursor -= 1;
            self.ensure_cursor_visible();
        }

        if self.selection_mode == SelectionMode::Range {
            self.update_range_selection();
        }
    }

    /// Jump to first entry
    pub fn cursor_top(&mut self) {
        self.push_undo(UndoAction::CursorMove { from: self.cursor }, "cursor");
        self.cursor = 0;
        self.scroll_offset = 0;
    }

    /// Jump to last entry
    pub fn cursor_bottom(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.push_undo(UndoAction::CursorMove { from: self.cursor }, "cursor");
        self.cursor = self.entries.len() - 1;
        self.ensure_cursor_visible();
    }

    /// Toggle selection on current entry
    pub fn toggle_selection(&mut self) {
        self.push_undo(
            UndoAction::Selection {
                previous: self.selected.clone(),
            },
            "selection",
        );

        if self.selected.contains(&self.cursor) {
            self.selected.retain(|&i| i != self.cursor);
        } else {
            self.selected.push(self.cursor);
        }
    }

    /// Select all entries
    pub fn select_all(&mut self) {
        self.push_undo(
            UndoAction::Selection {
                previous: self.selected.clone(),
            },
            "selection",
        );
        self.selected = (0..self.entries.len()).collect();
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        if !self.selected.is_empty() {
            self.push_undo(
                UndoAction::Selection {
                    previous: self.selected.clone(),
                },
                "selection",
            );
            self.selected.clear();
        }
    }

    /// Start range selection
    pub fn start_range_selection(&mut self) {
        self.selection_mode = SelectionMode::Range;
        self.selection_anchor = Some(self.cursor);
        self.selected = vec![self.cursor];
    }

    /// Enter search mode
    pub fn enter_search_mode(&mut self) {
        self.input_mode = InputMode::Search;
        self.search_query.clear();
    }

    /// Exit current input mode
    pub fn exit_input_mode(&mut self) {
        self.input_mode = InputMode::Normal;
        self.selection_mode = SelectionMode::Single;
        self.selection_anchor = None;
    }

    /// Set status message
    pub fn set_status(&mut self, message: impl Into<String>, level: StatusLevel) {
        self.status_message = Some((message.into(), level));
    }

    /// Clear status message
    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    /// Get currently selected paths
    pub fn selected_paths(&self) -> Vec<&str> {
        if self.selected.is_empty() {
            // If nothing selected, use cursor position
            self.entries
                .get(self.cursor)
                .map(|e| vec![e.path.as_str()])
                .unwrap_or_default()
        } else {
            self.selected
                .iter()
                .filter_map(|&i| self.entries.get(i).map(|e| e.path.as_str()))
                .collect()
        }
    }

    /// Get entry at cursor
    pub fn current_entry(&self) -> Option<&FileEntry> {
        self.entries.get(self.cursor)
    }

    /// Parent path
    pub fn parent_path(&self) -> Option<String> {
        if self.current_path == "/" {
            return None;
        }
        let path = self.current_path.trim_end_matches('/');
        path.rsplit_once('/')
            .map(|(parent, _)| {
                if parent.is_empty() {
                    "/".to_string()
                } else {
                    parent.to_string()
                }
            })
    }

    /// Undo last action
    pub fn undo(&mut self) -> bool {
        if let Some(entry) = self.undo_stack.pop_back() {
            let redo_action = match &entry.action {
                UndoAction::Navigate { from } => {
                    let current = self.current_path.clone();
                    self.current_path = from.clone();
                    UndoAction::Navigate { from: current }
                }
                UndoAction::CursorMove { from } => {
                    let current = self.cursor;
                    self.cursor = *from;
                    self.ensure_cursor_visible();
                    UndoAction::CursorMove { from: current }
                }
                UndoAction::Selection { previous } => {
                    let current = self.selected.clone();
                    self.selected = previous.clone();
                    UndoAction::Selection { previous: current }
                }
            };

            self.redo_stack.push(UndoEntry {
                action: redo_action,
                description: entry.description,
            });
            true
        } else {
            false
        }
    }

    /// Redo last undone action
    pub fn redo(&mut self) -> bool {
        if let Some(entry) = self.redo_stack.pop() {
            let undo_action = match &entry.action {
                UndoAction::Navigate { from } => {
                    let current = self.current_path.clone();
                    self.current_path = from.clone();
                    UndoAction::Navigate { from: current }
                }
                UndoAction::CursorMove { from } => {
                    let current = self.cursor;
                    self.cursor = *from;
                    self.ensure_cursor_visible();
                    UndoAction::CursorMove { from: current }
                }
                UndoAction::Selection { previous } => {
                    let current = self.selected.clone();
                    self.selected = previous.clone();
                    UndoAction::Selection { previous: current }
                }
            };

            self.undo_stack.push_back(UndoEntry {
                action: undo_action,
                description: entry.description,
            });
            true
        } else {
            false
        }
    }

    // Private helpers

    fn push_undo(&mut self, action: UndoAction, description: &str) {
        self.redo_stack.clear();
        self.undo_stack.push_back(UndoEntry {
            action,
            description: description.to_string(),
        });

        if self.undo_stack.len() > MAX_UNDO_HISTORY {
            self.undo_stack.pop_front();
        }
    }

    fn ensure_cursor_visible(&mut self) {
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        } else if self.cursor >= self.scroll_offset + self.visible_rows {
            self.scroll_offset = self.cursor - self.visible_rows + 1;
        }
    }

    fn update_range_selection(&mut self) {
        if let Some(anchor) = self.selection_anchor {
            let start = anchor.min(self.cursor);
            let end = anchor.max(self.cursor);
            self.selected = (start..=end).collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<FileEntry> {
        (0..10)
            .map(|i| FileEntry {
                name: format!("file{}.txt", i),
                path: format!("/test/file{}.txt", i),
                file_type: FileType::File,
                size: 100,
                modified: 0,
                mime_type: None,
            })
            .collect()
    }

    #[test]
    fn test_cursor_movement() {
        let mut state = AppState::default();
        state.entries = sample_entries();

        state.cursor_down();
        assert_eq!(state.cursor, 1);

        state.cursor_down();
        state.cursor_down();
        assert_eq!(state.cursor, 3);

        state.cursor_up();
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn test_undo_redo() {
        let mut state = AppState::default();
        state.entries = sample_entries();

        state.cursor_down();
        state.cursor_down();
        assert_eq!(state.cursor, 2);

        state.undo();
        assert_eq!(state.cursor, 1);

        state.redo();
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn test_selection() {
        let mut state = AppState::default();
        state.entries = sample_entries();

        state.toggle_selection();
        assert!(state.selected.contains(&0));

        state.cursor_down();
        state.toggle_selection();
        assert_eq!(state.selected.len(), 2);

        state.toggle_selection();
        assert_eq!(state.selected.len(), 1);
    }

    #[test]
    fn test_range_selection() {
        let mut state = AppState::default();
        state.entries = sample_entries();

        state.cursor = 2;
        state.start_range_selection();

        state.cursor = 5;
        state.update_range_selection();

        assert_eq!(state.selected, vec![2, 3, 4, 5]);
    }
}
