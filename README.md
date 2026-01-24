# pibox - Embedded Device Bootstrapping

A Rust workspace for managing and orchestrating embedded devices (NAS, cameras, sensors) via WebSocket communication. Provides both TUI and GUI clients.

## Architecture

```
┌─────────────┐     WebSocket      ┌─────────────────┐
│  pibox-tui  │◄──────────────────►│  pibox-server   │
│  pibox-gui  │                    │  (on device)    │
└─────────────┘                    └────────┬────────┘
                                            │ REST
                                   ┌────────▼────────┐
                                   │   Filebrowser   │
                                   │    backend      │
                                   └─────────────────┘
```

## Crates

| Crate | Description |
|-------|-------------|
| `pibox-core` | Shared library: protocol types, JWT auth, state machine, config |
| `pibox-server` | WebSocket server that runs on embedded devices |
| `pibox-tui` | Terminal UI client with vim keybindings |
| `pibox-gui` | Iced-based graphical client |

## Features

- **WebSocket protocol** - Real-time bidirectional communication
- **JWT authentication** - Access and refresh tokens with secure rotation
- **File management** - Browse, upload, download via Filebrowser backend
- **State machine** - Undo/redo support for file operations
- **Multi-device** - Manage multiple devices from one client
- **Cross-platform** - Linux, macOS, Windows client support

## Configuration

Config is stored in platform-appropriate locations:
- Linux: `~/.config/pibox/config.toml`
- macOS: `~/Library/Application Support/pibox/config.toml`
- Windows: `%APPDATA%\pibox\config.toml`

Example:
```toml
[server]
listen_addr = "0.0.0.0"
ws_port = 9280
filebrowser_url = "http://127.0.0.1:8080"

[client]
theme = "dark"
vim_mode = true
confirm_delete = true

[[devices]]
name = "nas"
url = "ws://192.0.2.10:9280"
device_type = "nas"
```

## Building

```bash
# Build all crates
cargo build --release

# Build specific crate
cargo build -p pibox-tui --release
```

## Running

```bash
# Server (on embedded device)
./pibox-server

# TUI client
./pibox-tui

# GUI client
./pibox-gui
```

## Default Ports

- WebSocket server: `9280`
- Filebrowser backend: `8080` (localhost only)

## License

MIT
