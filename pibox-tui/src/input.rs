//! Keyboard input handling with vim-style bindings

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pibox_core::state::InputMode;

use crate::app::{App, AppResult};

/// Handle a key event
pub async fn handle_key(app: &mut App, key: KeyEvent) -> AppResult {
    match app.state.input_mode {
        InputMode::Normal => handle_normal_mode(app, key).await,
        InputMode::Search => handle_search_mode(app, key),
        InputMode::Command => handle_command_mode(app, key),
        InputMode::Rename => handle_rename_mode(app, key),
        InputMode::Confirm(_) => handle_confirm_mode(app, key),
    }
}

/// Handle keys in normal mode (main navigation)
async fn handle_normal_mode(app: &mut App, key: KeyEvent) -> AppResult {
    match key.code {
        // Navigation (vim-style)
        KeyCode::Char('j') | KeyCode::Down => {
            app.state.cursor_down();
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.state.cursor_up();
        }
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => {
            app.navigate_up().await;
        }
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
            app.enter().await;
        }

        // Jump navigation
        KeyCode::Char('g') => {
            // gg = go to top (would need state for multi-key)
            app.state.cursor_top();
        }
        KeyCode::Char('G') => {
            // G = go to bottom
            app.state.cursor_bottom();
        }

        // Page navigation
        KeyCode::PageDown | KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            for _ in 0..app.state.visible_rows {
                app.state.cursor_down();
            }
        }
        KeyCode::PageUp | KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            for _ in 0..app.state.visible_rows {
                app.state.cursor_up();
            }
        }

        // Selection
        KeyCode::Char(' ') => {
            app.state.toggle_selection();
            app.state.cursor_down(); // Move to next after toggle
        }
        KeyCode::Char('V') => {
            // Visual line mode (range selection)
            app.state.start_range_selection();
        }
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.state.select_all();
        }

        // Actions
        KeyCode::Char('y') => {
            // Yank (copy)
            app.copy_selected();
        }
        KeyCode::Char('p') => {
            // Paste
            app.paste().await;
        }
        KeyCode::Char('d') => {
            // Delete
            app.delete_selected().await;
        }
        KeyCode::Char('r') => {
            // Rename
            app.state.input_mode = InputMode::Rename;
        }

        // Mode switching
        KeyCode::Char('/') => {
            app.state.enter_search_mode();
        }
        KeyCode::Char(':') => {
            app.state.input_mode = InputMode::Command;
            app.state.command_input.clear();
        }

        // Undo/Redo
        KeyCode::Char('u') => {
            app.state.undo();
        }
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.state.redo();
        }

        // Escape clears selection
        KeyCode::Esc => {
            app.state.clear_selection();
            app.state.clear_status();
        }

        // Help
        KeyCode::Char('?') => {
            app.state.set_status(
                "j/k:move h/l:nav space:select d:del y:copy p:paste /:search q:quit",
                pibox_core::state::StatusLevel::Info,
            );
        }

        _ => {}
    }

    AppResult::Continue
}

/// Handle keys in search mode
fn handle_search_mode(app: &mut App, key: KeyEvent) -> AppResult {
    match key.code {
        KeyCode::Esc => {
            app.state.exit_input_mode();
        }
        KeyCode::Enter => {
            // Execute search
            let query = app.state.search_query.clone();
            app.state.set_status(format!("Search: {}", query), pibox_core::state::StatusLevel::Info);
            app.state.exit_input_mode();
        }
        KeyCode::Backspace => {
            app.state.search_query.pop();
        }
        KeyCode::Char(c) => {
            app.state.search_query.push(c);
        }
        _ => {}
    }

    AppResult::Continue
}

/// Handle keys in command mode
fn handle_command_mode(app: &mut App, key: KeyEvent) -> AppResult {
    match key.code {
        KeyCode::Esc => {
            app.state.exit_input_mode();
        }
        KeyCode::Enter => {
            let cmd = app.state.command_input.clone();
            execute_command(app, &cmd);
            app.state.exit_input_mode();
        }
        KeyCode::Backspace => {
            app.state.command_input.pop();
        }
        KeyCode::Char(c) => {
            app.state.command_input.push(c);
        }
        _ => {}
    }

    AppResult::Continue
}

/// Handle keys in rename mode
fn handle_rename_mode(app: &mut App, key: KeyEvent) -> AppResult {
    match key.code {
        KeyCode::Esc => {
            app.state.exit_input_mode();
        }
        KeyCode::Enter => {
            // TODO: Execute rename
            app.state.exit_input_mode();
        }
        _ => {}
    }

    AppResult::Continue
}

/// Handle keys in confirmation mode
fn handle_confirm_mode(app: &mut App, key: KeyEvent) -> AppResult {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            // TODO: Execute confirmed action
            app.state.exit_input_mode();
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.state.exit_input_mode();
        }
        _ => {}
    }

    AppResult::Continue
}

/// Execute a command-mode command
fn execute_command(app: &mut App, cmd: &str) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    match parts.first().copied() {
        Some("q") | Some("quit") => {
            // Will be caught in main loop
        }
        Some("w") | Some("write") => {
            app.state.set_status("Nothing to save", pibox_core::state::StatusLevel::Info);
        }
        Some("wq") => {
            // Save and quit
        }
        Some("cd") => {
            if let Some(path) = parts.get(1) {
                app.state.set_status(format!("cd {}", path), pibox_core::state::StatusLevel::Info);
            }
        }
        Some("set") => {
            if let Some(opt) = parts.get(1) {
                app.state.set_status(format!("set {}", opt), pibox_core::state::StatusLevel::Info);
            }
        }
        Some(unknown) => {
            app.state.set_status(
                format!("Unknown command: {}", unknown),
                pibox_core::state::StatusLevel::Error,
            );
        }
        None => {}
    }
}
