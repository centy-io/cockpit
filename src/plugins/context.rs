//! Plugin context - provides plugins with access to cockpit state.

use std::path::PathBuf;

use crate::pane::PaneId;

/// Context provided to plugins for accessing cockpit state.
///
/// This is read-only for now. Future versions may add write capabilities
/// for interactive plugins.
#[derive(Clone, Debug)]
pub struct PluginContext {
    /// Current working directory (for git plugins, etc.)
    pub cwd: PathBuf,
    /// Currently focused pane ID, if any.
    pub focused_pane: Option<PaneId>,
    /// Number of active panes.
    pub pane_count: usize,
    /// Terminal width.
    pub terminal_width: u16,
}

impl PluginContext {
    /// Create a new plugin context.
    #[must_use]
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            focused_pane: None,
            pane_count: 0,
            terminal_width: 80,
        }
    }

    /// Update context from `PaneManager` state.
    pub fn update(&mut self, focused: Option<PaneId>, pane_count: usize, width: u16) {
        self.focused_pane = focused;
        self.pane_count = pane_count;
        self.terminal_width = width;
    }
}
