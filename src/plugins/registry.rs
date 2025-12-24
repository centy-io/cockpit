//! Plugin registry - manages plugin lifecycle and refresh scheduling.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::pane::PaneId;
use crate::plugins::context::PluginContext;
use crate::plugins::{Plugin, PluginConfig, PluginError, PluginId, PluginResult};
use crate::status_bar::StatusBarSegment;

/// Internal representation of a registered plugin.
struct RegisteredPlugin {
    plugin: Box<dyn Plugin>,
    config: PluginConfig,
    last_refresh: Instant,
    cached_segment: StatusBarSegment,
}

/// Registry for managing plugins.
pub struct PluginRegistry {
    plugins: HashMap<PluginId, RegisteredPlugin>,
    next_id: AtomicU64,
    context: PluginContext,
}

impl PluginRegistry {
    /// Create a new plugin registry.
    #[must_use]
    pub fn new(cwd: std::path::PathBuf) -> Self {
        Self {
            plugins: HashMap::new(),
            next_id: AtomicU64::new(1),
            context: PluginContext::new(cwd),
        }
    }

    /// Register a plugin.
    ///
    /// # Errors
    /// Returns an error if plugin initialization or initial refresh fails.
    pub fn register(&mut self, mut plugin: Box<dyn Plugin>) -> PluginResult<PluginId> {
        let id = PluginId(self.next_id.fetch_add(1, Ordering::SeqCst));
        let config = plugin.config();

        // Initialize plugin
        plugin.init(&self.context)?;

        // Initial refresh
        plugin.refresh(&self.context)?;
        let segment = plugin.render();

        self.plugins.insert(
            id,
            RegisteredPlugin {
                plugin,
                config,
                last_refresh: Instant::now(),
                cached_segment: segment,
            },
        );

        Ok(id)
    }

    /// Unregister a plugin.
    ///
    /// # Errors
    /// Returns an error if the plugin is not found.
    pub fn unregister(&mut self, id: PluginId) -> PluginResult<()> {
        let mut registered = self.plugins.remove(&id).ok_or(PluginError::NotFound(id))?;
        registered.plugin.shutdown();
        Ok(())
    }

    /// Update context from manager state.
    pub fn update_context(&mut self, focused: Option<PaneId>, pane_count: usize, width: u16) {
        self.context.update(focused, pane_count, width);
    }

    /// Tick all plugins - refresh those that need it.
    pub fn tick(&mut self) {
        let now = Instant::now();

        for registered in self.plugins.values_mut() {
            let elapsed = now.duration_since(registered.last_refresh);
            if elapsed >= registered.config.refresh_interval {
                // Refresh plugin
                if registered.plugin.refresh(&self.context).is_ok() {
                    registered.cached_segment = registered.plugin.render();
                }
                registered.last_refresh = now;
            }
        }
    }

    /// Get all segments for rendering, sorted by priority.
    #[must_use]
    pub fn segments(&self) -> Vec<&StatusBarSegment> {
        let mut entries: Vec<_> = self.plugins.values().collect();
        entries.sort_by_key(|r| r.config.priority);
        entries.iter().map(|r| &r.cached_segment).collect()
    }
}
