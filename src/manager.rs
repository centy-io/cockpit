//! Pane manager - central orchestrator for all panes.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;

use crate::error::{Error, Result};
use crate::layout::{Layout, LayoutCalculator};
use crate::pane::{PaneHandle, PaneId, PaneSize, SpawnConfig};
use crate::pty::{self, PaneEvent, SpawnedPty};

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
            max_panes: 64,
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
    /// Current layout.
    layout: Option<Layout>,
    /// Currently focused pane.
    focused: Option<PaneId>,
    /// Event sender for pane events.
    event_tx: mpsc::Sender<PaneEvent>,
    /// Event receiver for pane events.
    event_rx: mpsc::Receiver<PaneEvent>,
    /// Next pane ID.
    next_id: AtomicU64,
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
        Self {
            config,
            panes: HashMap::new(),
            layout: None,
            focused: None,
            event_tx,
            event_rx,
            next_id: AtomicU64::new(1),
        }
    }

    /// Spawn a new pane with the given configuration.
    ///
    /// # Errors
    /// Returns an error if pane spawning fails or max panes is reached.
    pub fn spawn(&mut self, config: SpawnConfig) -> Result<PaneHandle> {
        if self.panes.len() >= self.config.max_panes {
            return Err(Error::Layout(format!(
                "Maximum panes ({}) reached",
                self.config.max_panes
            )));
        }

        let pane_id = PaneId(self.next_id.fetch_add(1, Ordering::SeqCst));

        let mut spawn_config = config;
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

        // Auto-focus first pane
        if self.focused.is_none() {
            self.focused = Some(pane_id);
        }

        // Auto-set layout if this is the first pane
        if self.layout.is_none() {
            self.layout = Some(Layout::single(pane_id));
        }

        Ok(handle)
    }

    /// Get the current layout.
    #[must_use]
    pub fn layout(&self) -> Option<&Layout> {
        self.layout.as_ref()
    }

    /// Set the layout.
    pub fn set_layout(&mut self, layout: Layout) {
        self.layout = Some(layout);
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
        self.layout
            .as_ref()
            .map_or_else(HashMap::new, |layout| {
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
    pub fn close_pane(&mut self, pane_id: PaneId) {
        if let Some(managed) = self.panes.remove(&pane_id) {
            // Abort tasks
            managed.reader_handle.abort();
            managed.writer_handle.abort();
            managed.monitor_handle.abort();
        }

        // Update focus if needed
        if self.focused == Some(pane_id) {
            self.focused = self.panes.keys().next().copied();
        }
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
    pub fn pane_at_position(&self, x: u16, y: u16, areas: &HashMap<PaneId, Rect>) -> Option<PaneId> {
        for (pane_id, rect) in areas {
            if x >= rect.x
                && x < rect.x + rect.width
                && y >= rect.y
                && y < rect.y + rect.height
            {
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

    /// Convert the manager into a shared reference.
    #[must_use]
    pub fn into_shared(self) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(self))
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
