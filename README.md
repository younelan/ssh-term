# terminal-ssh

A GTK4 SSH terminal client written in Rust.

## Features

- **Themes**: Support for Light, Dark, Coffee, Dark Blue, and custom session-specific colors.
- **Multiple Tabs**: Functional close buttons, tab reordering via drag-and-drop, and automatic window closure.
- **Image Protocol Support**: High-performance rendering for **iTerm2** (OSC 1337), **Kitty** (OSC 108), and **Sixel** graphics.
- **Advanced Search**: Integrated search bar (`Ctrl+F` / `Cmd+F`) with forward/backward navigation.
- **Unicode & BiDi**: Full support for UTF-8, double-width characters, emojis, and right-to-left (RTL) text.
- **OSC 8 Hyperlinks**: Clickable URI support with hover highlighting.
- **Visual & Audible Bell**: Integrated system beep and visual background flash support.
- **PTY Resizing**: Exact dimension synchronization between the GTK view and the remote PTY.
- **Port Forwarding**: Local and remote port forwarding support (-L and -R).
- **Mouse Tracking**: Supports mouse clicks and scroll events for TUI applications.
- **Customization**: Individual session settings for fonts, colors, and terminal type.

## Requirements

- Rust and Cargo (latest stable)
- GTK4 development libraries

## Running

To build and run the application:

```bash
cargo run --release
```

## Configuration

Settings are stored in `~/.terminal_ssh_sessions.json` and global app preferences in `~/.terminal_ssh_app_config.json`.
