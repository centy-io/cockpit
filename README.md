# Cockpit

A terminal multiplexer library for [Ratatui](https://ratatui.rs) applications.

Cockpit enables running multiple OS processes in split panes with crash isolation. Each pane runs in its own PTY (pseudo-terminal), so if one process crashes, the others continue running unaffected.

## Features

- **PTY Management**: Spawn processes in pseudo-terminals using `portable-pty`
- **Terminal Emulation**: Full VT100/ANSI terminal emulation via `vt100`
- **Split Layouts**: Horizontal and vertical pane splits
- **Crash Isolation**: Each process runs independently
- **Ratatui Integration**: Widgets for rendering panes
- **Mouse Support**: Click to focus panes

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
cockpit = "0.1"
```

## Quick Start

```rust
use cockpit::{PaneManager, SpawnConfig, PaneSize, Layout};

#[tokio::main]
async fn main() -> cockpit::Result<()> {
    // Create a pane manager
    let mut manager = PaneManager::new();

    // Spawn two panes
    let size = PaneSize::new(24, 80);
    let pane1 = manager.spawn(SpawnConfig::new(size))?;
    let pane2 = manager.spawn(SpawnConfig::new(size))?;

    // Set up a vertical split layout
    let layout = Layout::vsplit_equal(
        Layout::single(pane1.id()),
        Layout::single(pane2.id()),
    );
    manager.set_layout(layout);

    // Send input to the focused pane
    manager.send_input(b"echo hello\r").await?;

    Ok(())
}
```

## Example

Run the interactive example:

```bash
cargo run --example basic
```

Controls:
- **Ctrl+Q**: Quit
- **Ctrl+N**: Focus next pane
- **Mouse click**: Focus pane under cursor

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
