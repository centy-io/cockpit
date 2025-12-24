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
//! - **Split Layouts**: Horizontal and vertical pane splits
//! - **Crash Isolation**: Each process runs independently
//! - **Ratatui Integration**: Widgets for rendering panes
//!
//! ## Example
//!
//! ```no_run
//! use cockpit::{PaneManager, SpawnConfig, PaneSize, Layout};
//!
//! #[tokio::main]
//! async fn main() -> cockpit::Result<()> {
//!     // Create a pane manager
//!     let mut manager = PaneManager::new();
//!
//!     // Spawn two panes
//!     let size = PaneSize::new(24, 80);
//!     let pane1 = manager.spawn(SpawnConfig::new(size))?;
//!     let pane2 = manager.spawn(SpawnConfig::new(size))?;
//!
//!     // Set up a vertical split layout
//!     let layout = Layout::vsplit_equal(
//!         Layout::single(pane1.id()),
//!         Layout::single(pane2.id()),
//!     );
//!     manager.set_layout(layout);
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

mod error;
mod layout;
mod manager;
mod pane;
mod pty;
mod widget;

// Re-export public API
pub use error::{Error, Result};
pub use layout::{Direction, Layout};
pub use manager::{ManagerConfig, PaneManager};
pub use pane::{
    PaneHandle, PaneId, PaneSize, PaneState, ScreenCell, ScreenColor, ScreenSnapshot, SpawnConfig,
};
pub use pty::PaneEvent;
pub use widget::{CockpitWidget, ConfirmDialog, DialogButton, DialogState, PaneWidget};
