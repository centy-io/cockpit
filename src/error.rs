//! Error types for the cockpit crate.

use thiserror::Error;

/// Result type alias using cockpit's Error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in cockpit operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to spawn a PTY process.
    #[error("failed to spawn PTY: {0}")]
    PtySpawn(#[from] std::io::Error),

    /// Anyhow error from portable-pty.
    #[error("PTY error: {0}")]
    Pty(#[from] anyhow::Error),

    /// Failed to create PTY pair.
    #[error("failed to create PTY: {0}")]
    PtyCreate(String),

    /// The pane has been closed.
    #[error("pane has been closed")]
    PaneClosed,

    /// Pane not found with the given ID.
    #[error("pane not found: {0}")]
    PaneNotFound(u64),

    /// Layout error.
    #[error("layout error: {0}")]
    Layout(String),

    /// PTY resize failed.
    #[error("failed to resize PTY: {0}")]
    Resize(String),

    /// Input send failed.
    #[error("failed to send input to pane")]
    InputSend,

    /// Process monitoring error.
    #[error("process monitor error: {0}")]
    ProcessMonitor(String),
}
