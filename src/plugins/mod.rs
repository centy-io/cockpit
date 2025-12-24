//! Plugin system for cockpit status bar.
//!
//! Plugins provide content for the status bar. They can be display-only
//! (current implementation) or interactive (future capability).

mod context;
mod git_user;
mod registry;

pub use context::PluginContext;
pub use git_user::GitUserPlugin;
pub use registry::PluginRegistry;

use std::time::Duration;

use crate::status_bar::StatusBarSegment;

/// Unique identifier for a plugin instance.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct PluginId(pub u64);

impl std::fmt::Display for PluginId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Configuration for plugin behavior.
#[derive(Clone, Debug)]
pub struct PluginConfig {
    /// How often to refresh the plugin's data.
    pub refresh_interval: Duration,
    /// Position in the status bar (lower = more left).
    pub priority: i32,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            refresh_interval: Duration::from_secs(5),
            priority: 0,
        }
    }
}

/// The core plugin trait.
///
/// Plugins provide content for the status bar. They can be display-only
/// (current implementation) or interactive (future capability).
pub trait Plugin: Send + Sync {
    /// Unique name for this plugin type.
    fn name(&self) -> &'static str;

    /// Plugin configuration.
    fn config(&self) -> PluginConfig {
        PluginConfig::default()
    }

    /// Initialize the plugin. Called once when registered.
    ///
    /// # Errors
    /// Returns an error if initialization fails.
    fn init(&mut self, ctx: &PluginContext) -> PluginResult<()> {
        let _ = ctx;
        Ok(())
    }

    /// Refresh the plugin's data.
    /// This is called periodically based on `refresh_interval`.
    ///
    /// # Errors
    /// Returns an error if refresh fails.
    fn refresh(&mut self, ctx: &PluginContext) -> PluginResult<()>;

    /// Render the plugin's status bar segment.
    fn render(&self) -> StatusBarSegment;

    /// Cleanup when plugin is removed. Called once.
    fn shutdown(&mut self) {
        // Default: no cleanup needed
    }
}

/// Result type for plugin operations.
pub type PluginResult<T> = Result<T, PluginError>;

/// Errors that can occur in plugin operations.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// Plugin initialization failed.
    #[error("plugin initialization failed: {0}")]
    InitFailed(String),

    /// Plugin refresh failed.
    #[error("plugin refresh failed: {0}")]
    RefreshFailed(String),

    /// Command execution failed.
    #[error("command execution failed: {0}")]
    CommandFailed(#[from] std::io::Error),

    /// Plugin not found.
    #[error("plugin not found: {0}")]
    NotFound(PluginId),
}
