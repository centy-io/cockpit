//! Pane manager - central orchestrator for all panes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;

use crate::arrows::{down_arrow_at_position, horizontal_arrow_at_position, up_arrow_at_position};
use crate::error::{Error, Result};
use crate::layout::{Layout, LayoutCalculator};
use crate::pane::{PaneHandle, PaneId, PaneSize, SpawnConfig};
use crate::plugins::{Plugin, PluginId, PluginRegistry, PluginResult};
use crate::pty::{self, PaneEvent, SpawnedPty};
use crate::status_bar::StatusBarSegment;

/// Configuration for the pane manager.
#[derive(Clone, Debug)]
pub struct ManagerConfig {
    /// Maximum number of panes.
    pub max_panes: usize,
    /// Default scrollback buffer size.
    pub scrollback_lines: usize,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            max_panes: 4,
            scrollback_lines: 10_000,
        }
    }
}

/// Internal representation of a managed pane.
struct ManagedPane {
    /// The public handle.
    handle: PaneHandle,
    /// PTY master for resize operations.
    pty_master: Box<dyn portable_pty::MasterPty + Send>,
    /// Reader task handle.
    #[allow(dead_code)]
    reader_handle: JoinHandle<()>,
    /// Writer task handle.
    #[allow(dead_code)]
    writer_handle: JoinHandle<()>,
    /// Monitor task handle.
    #[allow(dead_code)]
    monitor_handle: JoinHandle<()>,
}

/// Central manager for all panes.
pub struct PaneManager {
    /// Configuration.
    config: ManagerConfig,
    /// All active panes.
    panes: HashMap<PaneId, ManagedPane>,
    /// Current layout (internal, automatically managed).
    layout: Option<Layout>,
    /// Currently focused pane.
    focused: Option<PaneId>,
    /// Event sender for pane events.
    event_tx: mpsc::Sender<PaneEvent>,
    /// Event receiver for pane events.
    event_rx: mpsc::Receiver<PaneEvent>,
    /// Next pane ID.
    next_id: AtomicU64,
    /// Plugin registry for status bar plugins.
    plugin_registry: Option<PluginRegistry>,
    /// Current terminal size for automatic layout calculations.
    terminal_size: Option<Rect>,
    /// Pre-calculated pane areas (updated on spawn/close/resize).
    cached_areas: HashMap<PaneId, Rect>,
    /// Order of panes for consistent layout (first = left, second = right).
    pane_order: Vec<PaneId>,
    /// Sub-pane areas (non-PTY decorative panes).
    sub_pane_areas: Vec<Rect>,
    /// Ratio of space for panes vs sub-panes (0.7 = 70% panes, 30% sub-panes).
    sub_pane_ratio: f32,
    /// Empty pane areas for slots without active PTYs (pane_number, Rect).
    empty_pane_areas: Vec<(usize, Rect)>,
    /// Which pane positions (0-3) are expanded (hiding their sub-panes).
    expanded_positions: [bool; 4],
    /// Horizontal expansion state per row.
    /// Index 0 = top row (110/120), Index 1 = bottom row (210/220).
    /// None = no expansion, Some(true) = left expanded, Some(false) = right expanded.
    horizontal_expanded: [Option<bool>; 2],
}

impl PaneManager {
    /// Create a new pane manager with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(ManagerConfig::default())
    }

    /// Create a new pane manager with custom configuration.
    #[must_use]
    pub fn with_config(config: ManagerConfig) -> Self {
        let (event_tx, event_rx) = mpsc::channel(256);
        // Enforce max_panes = 4
        let config = ManagerConfig {
            max_panes: config.max_panes.min(4),
            ..config
        };
        Self {
            config,
            panes: HashMap::new(),
            layout: None,
            focused: None,
            event_tx,
            event_rx,
            next_id: AtomicU64::new(1),
            plugin_registry: None,
            terminal_size: None,
            cached_areas: HashMap::new(),
            pane_order: Vec::with_capacity(4),
            sub_pane_areas: Vec::new(),
            sub_pane_ratio: 0.7,
            empty_pane_areas: Vec::new(),
            expanded_positions: [false; 4],
            horizontal_expanded: [None; 2],
        }
    }

    /// Spawn a new pane with the given configuration.
    ///
    /// The pane size is calculated automatically based on the current terminal
    /// size and number of panes. Layout is updated automatically.
    ///
    /// # Errors
    /// Returns an error if pane spawning fails or max panes (2) is reached.
    pub fn spawn(&mut self, config: SpawnConfig) -> Result<PaneHandle> {
        if self.panes.len() >= self.config.max_panes {
            return Err(Error::Layout(format!(
                "Maximum panes ({}) reached",
                self.config.max_panes
            )));
        }

        let pane_id = PaneId(self.next_id.fetch_add(1, Ordering::SeqCst));

        // Calculate initial size from terminal size
        let initial_size = self.calculate_initial_pane_size();

        let mut spawn_config = config;
        spawn_config.size = initial_size; // Override with calculated size
        if spawn_config.scrollback == 0 {
            spawn_config.scrollback = self.config.scrollback_lines;
        }

        let SpawnedPty {
            handle,
            pty_master,
            reader_handle,
            writer_handle,
            monitor_handle,
        } = pty::spawn_pty(pane_id, &spawn_config, self.event_tx.clone())?;

        let managed = ManagedPane {
            handle: handle.clone(),
            pty_master,
            reader_handle,
            writer_handle,
            monitor_handle,
        };

        self.panes.insert(pane_id, managed);
        self.pane_order.push(pane_id);

        // Auto-focus first pane
        if self.focused.is_none() {
            self.focused = Some(pane_id);
        }

        // Recalculate layout for new pane count
        self.recalculate_layout();

        // Resize all panes to their new areas (ignore errors during spawn)
        let _ = self.resize_all_panes();

        Ok(handle)
    }

    /// Get the currently focused pane ID.
    #[must_use]
    pub fn focused(&self) -> Option<PaneId> {
        self.focused
    }

    /// Set focus to a specific pane.
    pub fn set_focus(&mut self, pane_id: PaneId) {
        if self.panes.contains_key(&pane_id) {
            self.focused = Some(pane_id);
        }
    }

    /// Get a pane handle by ID.
    #[must_use]
    pub fn get_pane(&self, pane_id: PaneId) -> Option<&PaneHandle> {
        self.panes.get(&pane_id).map(|p| &p.handle)
    }

    /// Get all pane IDs.
    #[must_use]
    pub fn pane_ids(&self) -> Vec<PaneId> {
        self.panes.keys().copied().collect()
    }

    /// Get the number of panes.
    #[must_use]
    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    /// Calculate layout areas for the given total area.
    #[must_use]
    pub fn calculate_areas(&self, area: Rect) -> HashMap<PaneId, Rect> {
        self.layout.as_ref().map_or_else(HashMap::new, |layout| {
            LayoutCalculator::calculate_areas(layout, area)
        })
    }

    /// Resize a pane's PTY.
    ///
    /// # Errors
    /// Returns an error if the pane is not found or resize fails.
    pub fn resize_pane(&mut self, pane_id: PaneId, size: PaneSize) -> Result<()> {
        let managed = self
            .panes
            .get(&pane_id)
            .ok_or(Error::PaneNotFound(pane_id.0))?;
        pty::resize_pty(managed.pty_master.as_ref(), size)
    }

    /// Set the terminal size and initialize internal layout calculations.
    ///
    /// This should be called once at startup with the initial terminal size,
    /// before spawning any panes. Cockpit manages all layout internally.
    pub fn set_terminal_size(&mut self, size: Rect) {
        if self.terminal_size == Some(size) {
            return;
        }
        self.terminal_size = Some(size);
        self.recalculate_layout();
        let _ = self.resize_all_panes();
    }

    /// Get pre-calculated pane areas.
    ///
    /// These are updated automatically when panes are added/removed or on resize.
    #[must_use]
    pub fn get_areas(&self) -> &HashMap<PaneId, Rect> {
        &self.cached_areas
    }

    /// Get sub-pane areas for rendering.
    #[must_use]
    pub fn get_sub_pane_areas(&self) -> &[Rect] {
        &self.sub_pane_areas
    }

    /// Get empty pane areas for rendering (slots without active PTYs).
    #[must_use]
    pub fn get_empty_pane_areas(&self) -> &[(usize, Rect)] {
        &self.empty_pane_areas
    }

    /// Get which pane positions are expanded.
    #[must_use]
    pub fn get_expanded_positions(&self) -> &[bool; 4] {
        &self.expanded_positions
    }

    /// Toggle expansion state for a pane position (0-3).
    /// When expanded, the pane takes full height and its sub-panes are hidden.
    pub fn toggle_pane_expansion(&mut self, position: usize) {
        if position < 4 {
            self.expanded_positions[position] = !self.expanded_positions[position];
            self.recalculate_layout();
            let _ = self.resize_all_panes();
        }
    }

    /// Toggle horizontal expansion for a row.
    /// - row 0 = top row (panes 110/120)
    /// - row 1 = bottom row (panes 210/220)
    /// - expand_left = true means left pane expands, false means right pane expands
    pub fn toggle_horizontal_expansion(&mut self, row: usize, expand_left: bool) {
        if row < 2 {
            let current = self.horizontal_expanded[row];
            self.horizontal_expanded[row] = match current {
                None => Some(expand_left),
                Some(was_left) if was_left == expand_left => None, // Toggle off
                Some(_) => Some(expand_left),                      // Switch direction
            };
            self.recalculate_layout();
            let _ = self.resize_all_panes();
        }
    }

    /// Get horizontal expansion state.
    pub fn get_horizontal_expanded(&self) -> &[Option<bool>; 2] {
        &self.horizontal_expanded
    }

    /// Recalculate layout based on current panes and terminal size.
    /// Always calculates 4 pane areas (2x2 grid) for consistent 12-pane layout.
    fn recalculate_layout(&mut self) {
        let Some(full_area) = self.terminal_size else {
            return;
        };

        // Split the area into panes (top) and sub-panes (bottom)
        let panes_height = (f32::from(full_area.height) * self.sub_pane_ratio).round() as u16;
        let sub_panes_height = full_area.height.saturating_sub(panes_height);

        // Calculate sub-pane areas - overlap by 1 row so borders share the same line
        let sub_panes_area = Rect {
            x: full_area.x,
            y: full_area.y + panes_height.saturating_sub(1),
            width: full_area.width,
            height: sub_panes_height + 1,
        };
        self.recalculate_sub_panes(sub_panes_area);

        // Always calculate 4 areas in a horizontal row (side by side)
        let quarter_width = full_area.width / 4;
        let half_width = full_area.width / 2;

        // Calculate widths based on horizontal expansion state
        // Row 0 = positions 0,1 (panes 110,120), Row 1 = positions 2,3 (panes 210,220)
        let (width_0, width_1) = match self.horizontal_expanded[0] {
            None => (quarter_width, quarter_width),
            Some(true) => (half_width, 0),  // Left expanded
            Some(false) => (0, half_width), // Right expanded
        };
        let (width_2, width_3) = match self.horizontal_expanded[1] {
            None => (quarter_width, full_area.width - quarter_width * 3), // Account for rounding
            Some(true) => (half_width, 0),                                // Left expanded
            Some(false) => (0, half_width),                               // Right expanded
        };

        // Calculate x positions based on widths
        let x_1 = full_area.x + width_0;
        let x_2 = full_area.x + half_width;
        let x_3 = full_area.x + half_width + width_2;

        // Calculate all 4 pane slot areas (positions 0-3, left to right)
        // Expanded panes get full height, others get panes_height
        let all_areas: [Rect; 4] = [
            // Position 0: leftmost (pane 110)
            Rect {
                x: full_area.x,
                y: full_area.y,
                width: width_0,
                height: if self.expanded_positions[0] {
                    full_area.height
                } else {
                    panes_height
                },
            },
            // Position 1: second from left (pane 120)
            Rect {
                x: x_1,
                y: full_area.y,
                width: width_1,
                height: if self.expanded_positions[1] {
                    full_area.height
                } else {
                    panes_height
                },
            },
            // Position 2: third from left (pane 210)
            Rect {
                x: x_2,
                y: full_area.y,
                width: width_2,
                height: if self.expanded_positions[2] {
                    full_area.height
                } else {
                    panes_height
                },
            },
            // Position 3: rightmost (pane 220)
            Rect {
                x: x_3,
                y: full_area.y,
                width: width_3,
                height: if self.expanded_positions[3] {
                    full_area.height
                } else {
                    panes_height
                },
            },
        ];

        // Clear and recalculate
        self.cached_areas.clear();
        self.empty_pane_areas.clear();

        // Assign active panes to positions, track empty slots
        for (i, area) in all_areas.iter().enumerate() {
            if i < self.pane_order.len() {
                let pane_id = self.pane_order[i];
                self.cached_areas.insert(pane_id, *area);
            } else {
                // Empty slot - store pane number (1-indexed)
                self.empty_pane_areas.push((i + 1, *area));
            }
        }

        // Update layout for active panes only (for internal use)
        // All panes are arranged horizontally (side by side)
        self.layout = match self.pane_order.len() {
            0 => None,
            1 => Some(Layout::single(self.pane_order[0])),
            2 => Some(Layout::hsplit_equal(
                Layout::single(self.pane_order[0]),
                Layout::single(self.pane_order[1]),
            )),
            3 => Some(Layout::hsplit_equal(
                Layout::single(self.pane_order[0]),
                Layout::hsplit_equal(
                    Layout::single(self.pane_order[1]),
                    Layout::single(self.pane_order[2]),
                ),
            )),
            4 => {
                // 4 panes in a horizontal row
                let left_half = Layout::hsplit_equal(
                    Layout::single(self.pane_order[0]),
                    Layout::single(self.pane_order[1]),
                );
                let right_half = Layout::hsplit_equal(
                    Layout::single(self.pane_order[2]),
                    Layout::single(self.pane_order[3]),
                );
                Some(Layout::hsplit_equal(left_half, right_half))
            }
            _ => None,
        };
    }

    /// Recalculate sub-pane areas.
    ///
    /// Creates sub-panes for non-expanded positions only.
    fn recalculate_sub_panes(&mut self, area: Rect) {
        self.sub_pane_areas.clear();

        // 8 sub-pane slots total (2 per pane position)
        // Sub-pane indices: 0-1 for position 0, 2-3 for position 1, 4-5 for position 2, 6-7 for position 3
        let quarter_width = area.width / 4;
        let half_width = area.width / 2;
        let eighth_width = area.width / 8;

        for i in 0..8 {
            let position = i / 2; // Which pane position (0-3) this sub-pane belongs to
            let row = position / 2; // Row 0 = positions 0,1; Row 1 = positions 2,3

            // Skip sub-panes for vertically expanded positions
            if self.expanded_positions[position] {
                self.sub_pane_areas.push(Rect::default());
                continue;
            }

            // Check horizontal expansion for this row
            let h_expanded = self.horizontal_expanded[row];

            // Determine if this sub-pane should be hidden due to horizontal expansion
            let is_hidden = match h_expanded {
                None => false,
                Some(true) => position == 1 || position == 3, // Right panes hidden
                Some(false) => position == 0 || position == 2, // Left panes hidden
            };

            if is_hidden {
                self.sub_pane_areas.push(Rect::default());
                continue;
            }

            // Calculate width and x position based on horizontal expansion
            let (x, width) = match h_expanded {
                None => {
                    // Normal layout: 8 sub-panes, each 1/8 width
                    let x = area.x + (i as u16 * eighth_width);
                    let width = if i == 7 {
                        area.width - (7 * eighth_width)
                    } else {
                        eighth_width
                    };
                    (x, width)
                }
                Some(true) => {
                    // Left pane expanded: sub-panes 0-1 or 4-5 take 1/4 each (total 1/2)
                    let local_idx = i % 2; // 0 or 1 within the pair
                    let base_x = if row == 0 {
                        area.x
                    } else {
                        area.x + half_width
                    };
                    let x = base_x + (local_idx as u16 * quarter_width);
                    (x, quarter_width)
                }
                Some(false) => {
                    // Right pane expanded: sub-panes 2-3 or 6-7 take 1/4 each (total 1/2)
                    let local_idx = i % 2; // 0 or 1 within the pair
                    let base_x = if row == 0 {
                        area.x
                    } else {
                        area.x + half_width
                    };
                    let x = base_x + (local_idx as u16 * quarter_width);
                    (x, quarter_width)
                }
            };

            self.sub_pane_areas.push(Rect {
                x,
                y: area.y,
                width,
                height: area.height,
            });
        }
    }

    /// Resize all panes to match their calculated areas.
    fn resize_all_panes(&mut self) -> Result<()> {
        for (pane_id, area) in &self.cached_areas {
            // Subtract 2 for border (1 on each side)
            let inner_width = area.width.saturating_sub(2);
            let inner_height = area.height.saturating_sub(2);

            if let Some(managed) = self.panes.get(pane_id) {
                let size = PaneSize::new(inner_height, inner_width);
                pty::resize_pty(managed.pty_master.as_ref(), size)?;
            }
        }
        Ok(())
    }

    /// Calculate initial pane size for spawning.
    fn calculate_initial_pane_size(&self) -> PaneSize {
        if let Some(mut area) = self.terminal_size {
            // Reduce available height for sub-panes
            area.height = (f32::from(area.height) * self.sub_pane_ratio).round() as u16;

            // Estimate size based on how many panes will exist
            let future_pane_count = self.panes.len() + 1;
            let (width, height) = match future_pane_count {
                1 => (area.width.saturating_sub(2), area.height.saturating_sub(2)),
                2 => (area.width / 2 - 1, area.height.saturating_sub(2)),
                3 | 4 => (area.width / 4 - 1, area.height.saturating_sub(2)),
                _ => (area.width / 4 - 1, area.height.saturating_sub(2)),
            };
            PaneSize::new(height, width)
        } else {
            // Default fallback size
            PaneSize::new(24, 80)
        }
    }

    /// Send input to the focused pane.
    ///
    /// # Errors
    /// Returns an error if no pane is focused or input sending fails.
    pub async fn send_input(&self, data: &[u8]) -> Result<()> {
        let pane_id = self.focused.ok_or(Error::PaneClosed)?;
        let managed = self
            .panes
            .get(&pane_id)
            .ok_or(Error::PaneNotFound(pane_id.0))?;
        managed.handle.send_input(data).await
    }

    /// Route a key event to the focused pane.
    ///
    /// # Errors
    /// Returns an error if input routing fails.
    pub async fn route_key(&self, key: KeyEvent) -> Result<()> {
        let bytes = key_to_bytes(key);
        if !bytes.is_empty() {
            self.send_input(&bytes).await?;
        }
        Ok(())
    }

    /// Poll for pane events without blocking.
    pub fn poll_events(&mut self) -> Vec<PaneEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Close a pane.
    ///
    /// Layout is automatically recalculated after closing.
    pub fn close_pane(&mut self, pane_id: PaneId) {
        if let Some(managed) = self.panes.remove(&pane_id) {
            // Abort tasks
            managed.reader_handle.abort();
            managed.writer_handle.abort();
            managed.monitor_handle.abort();
        }

        // Remove from pane_order
        self.pane_order.retain(|&id| id != pane_id);

        // Update focus if needed
        if self.focused == Some(pane_id) {
            self.focused = self.pane_order.first().copied();
        }

        // Recalculate layout
        self.recalculate_layout();

        // Resize remaining panes (ignore errors)
        let _ = self.resize_all_panes();
    }

    /// Cycle focus to the next pane.
    pub fn focus_next(&mut self) {
        let ids: Vec<_> = self.panes.keys().copied().collect();
        if ids.is_empty() {
            return;
        }

        let current = self.focused.unwrap_or(ids[0]);
        let pos = ids.iter().position(|&id| id == current).unwrap_or(0);
        let next_pos = (pos + 1) % ids.len();
        self.focused = Some(ids[next_pos]);
    }

    /// Cycle focus to the previous pane.
    pub fn focus_prev(&mut self) {
        let ids: Vec<_> = self.panes.keys().copied().collect();
        if ids.is_empty() {
            return;
        }

        let current = self.focused.unwrap_or(ids[0]);
        let pos = ids.iter().position(|&id| id == current).unwrap_or(0);
        let prev_pos = if pos == 0 { ids.len() - 1 } else { pos - 1 };
        self.focused = Some(ids[prev_pos]);
    }

    /// Find which pane contains the given screen coordinates.
    ///
    /// Returns the `PaneId` of the pane at position (x, y), or `None` if
    /// no pane contains that position.
    #[must_use]
    pub fn pane_at_position(
        &self,
        x: u16,
        y: u16,
        areas: &HashMap<PaneId, Rect>,
    ) -> Option<PaneId> {
        for (pane_id, rect) in areas {
            if x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height {
                return Some(*pane_id);
            }
        }
        None
    }

    /// Focus the pane at the given screen coordinates.
    ///
    /// Returns `true` if focus was changed, `false` if no pane was found
    /// at the position or if the clicked pane was already focused.
    pub fn focus_at_position(&mut self, x: u16, y: u16, areas: &HashMap<PaneId, Rect>) -> bool {
        if let Some(pane_id) = self.pane_at_position(x, y, areas) {
            if self.focused != Some(pane_id) {
                self.focused = Some(pane_id);
                return true;
            }
        }
        false
    }

    /// Handle a mouse click at the given screen coordinates.
    ///
    /// This is the unified click handler that:
    /// 1. First checks if clicking an up arrow on expanded pane → collapses pane
    /// 2. Then checks if clicking a down arrow on sub-pane → expands pane
    /// 3. Otherwise checks if clicking a pane → changes focus
    ///
    /// Returns `true` if any action was taken (expansion toggled or focus changed).
    pub fn handle_click(&mut self, x: u16, y: u16) -> bool {
        // First check for up arrow clicks on expanded panes (collapse)
        // Sort by x coordinate to ensure consistent position ordering (0-3 = left to right)
        let mut areas_vec: Vec<_> = self
            .cached_areas
            .iter()
            .map(|(&id, &rect)| (id, rect))
            .collect();
        areas_vec.sort_by_key(|(_, rect)| rect.x);
        if let Some(position) = up_arrow_at_position(x, y, &areas_vec, &self.expanded_positions) {
            self.toggle_pane_expansion(position);
            return true;
        }

        // Then check for down arrow clicks on sub-panes (expand)
        if let Some(arrow) = down_arrow_at_position(x, y, &self.sub_pane_areas) {
            self.toggle_pane_expansion(arrow.pane_position());
            return true;
        }

        // Check for horizontal arrow clicks (horizontal pane expansion)
        if let Some(arrow) = horizontal_arrow_at_position(x, y, &self.sub_pane_areas) {
            let source_position = arrow.source_position();
            let row = source_position / 2; // Row 0 = top, Row 1 = bottom
            let expand_left = source_position % 2 == 0; // Even positions are left panes
            self.toggle_horizontal_expansion(row, expand_left);
            return true;
        }

        // Otherwise handle pane focus
        let areas = self.cached_areas.clone();
        self.focus_at_position(x, y, &areas)
    }

    /// Convert the manager into a shared reference.
    #[must_use]
    pub fn into_shared(self) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(self))
    }

    /// Enable the plugin system with a working directory.
    #[must_use]
    pub fn with_plugins(mut self, cwd: PathBuf) -> Self {
        self.plugin_registry = Some(PluginRegistry::new(cwd));
        self
    }

    /// Register a plugin.
    ///
    /// # Errors
    /// Returns an error if plugins are not enabled or plugin registration fails.
    pub fn register_plugin(&mut self, plugin: Box<dyn Plugin>) -> PluginResult<PluginId> {
        self.plugin_registry
            .as_mut()
            .ok_or_else(|| {
                crate::plugins::PluginError::InitFailed("plugins not enabled".to_string())
            })?
            .register(plugin)
    }

    /// Tick plugins (call in main loop).
    pub fn tick_plugins(&mut self) {
        if let Some(registry) = &mut self.plugin_registry {
            registry.update_context(self.focused, self.panes.len(), 80);
            registry.tick();
        }
    }

    /// Get status bar segments for rendering.
    #[must_use]
    pub fn status_bar_segments(&self) -> Vec<&StatusBarSegment> {
        self.plugin_registry
            .as_ref()
            .map_or_else(Vec::new, PluginRegistry::segments)
    }
}

impl Default for PaneManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a key event to bytes to send to the PTY.
fn key_to_bytes(key: KeyEvent) -> Vec<u8> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    match key.code {
        KeyCode::Char(c) => {
            if ctrl {
                // Control characters (Ctrl+A = 0x01, etc.)
                let code = c.to_ascii_lowercase() as u8;
                if code.is_ascii_lowercase() {
                    vec![code - b'a' + 1]
                } else {
                    vec![]
                }
            } else if alt {
                // Alt sends ESC prefix
                vec![0x1b, c as u8]
            } else {
                c.to_string().into_bytes()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::F(n) => match n {
            1 => b"\x1bOP".to_vec(),
            2 => b"\x1bOQ".to_vec(),
            3 => b"\x1bOR".to_vec(),
            4 => b"\x1bOS".to_vec(),
            5 => b"\x1b[15~".to_vec(),
            6 => b"\x1b[17~".to_vec(),
            7 => b"\x1b[18~".to_vec(),
            8 => b"\x1b[19~".to_vec(),
            9 => b"\x1b[20~".to_vec(),
            10 => b"\x1b[21~".to_vec(),
            11 => b"\x1b[23~".to_vec(),
            12 => b"\x1b[24~".to_vec(),
            _ => vec![],
        },
        _ => vec![],
    }
}
