//! # Cockpit
//!
//! A terminal multiplexer library for Ratatui applications.
//!
//! Cockpit enables running multiple OS processes in split panes with crash isolation.
//! Each pane runs in its own PTY (pseudo-terminal), so if one process crashes,
//! the others continue running unaffected.
//!
//! ## Features
//!
//! - **PTY Management**: Spawn processes in pseudo-terminals using `portable-pty`
//! - **Terminal Emulation**: Full VT100/ANSI terminal emulation via `vt100`
//! - **Automatic Layout**: Side-by-side pane arrangement (max 4 panes)
//! - **Crash Isolation**: Each process runs independently
//! - **Ratatui Integration**: Widgets for rendering panes
//!
//! ## Example
//!
//! ```no_run
//! use cockpit::{PaneManager, SpawnConfig};
//! use ratatui::layout::Rect;
//!
//! #[tokio::main]
//! async fn main() -> cockpit::Result<()> {
//!     // Create a pane manager
//!     let mut manager = PaneManager::new();
//!
//!     // Set terminal size (get from your terminal)
//!     let term_size = Rect::new(0, 0, 120, 40);
//!     manager.set_terminal_size(term_size);
//!
//!     // Spawn panes - layout is automatic!
//!     manager.spawn(SpawnConfig::new_shell())?;  // Full screen
//!     manager.spawn(SpawnConfig::new_shell())?;  // Now 50/50 side-by-side
//!
//!     // Send input to the focused pane
//!     manager.send_input(b"echo hello\r").await?;
//!
//!     Ok(())
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod arrows;
mod error;
mod layout;
mod manager;
mod pane;
mod plugins;
mod pty;
mod status_bar;
mod widget;

// Re-export public API
pub use arrows::ArrowPosition;
pub use error::{Error, Result};
pub use manager::{ManagerConfig, PaneManager};
pub use pane::{
    PaneHandle, PaneId, PaneSize, PaneState, ScreenCell, ScreenColor, ScreenSnapshot, SpawnConfig,
};
pub use plugins::{
    GitUserPlugin, Plugin, PluginConfig, PluginContext, PluginError, PluginId, PluginRegistry,
    PluginResult,
};
pub use pty::PaneEvent;
pub use status_bar::{StatusBarConfig, StatusBarSegment, StatusBarWidget, STATUS_BAR_HEIGHT};
pub use widget::{
    CockpitWidget, ConfirmDialog, DialogButton, DialogState, PaneWidget, SubPaneWidget,
};
