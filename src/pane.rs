//! Pane types and handles for controlling terminal panes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use tokio::sync::{mpsc, watch};

use crate::error::{Error, Result};

/// Unique identifier for a pane.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct PaneId(pub u64);

impl std::fmt::Display for PaneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Current state of a pane's process.
#[derive(Clone, Debug)]
pub enum PaneState {
    /// Process is running.
    Running,

    /// Process exited normally with the given exit code.
    Exited { code: i32 },

    /// Process crashed or was killed by a signal.
    Crashed {
        /// Signal that killed the process (Unix only).
        signal: Option<i32>,
        /// Error description.
        error: Option<String>,
    },

    /// Pane is paused (process suspended).
    Paused,
}

impl PaneState {
    /// Returns true if the pane's process is still alive.
    #[must_use]
    pub fn is_alive(&self) -> bool {
        matches!(self, Self::Running | Self::Paused)
    }
}

/// Pane dimensions in rows and columns.
#[derive(Clone, Copy, Debug, Default)]
pub struct PaneSize {
    /// Number of rows.
    pub rows: u16,
    /// Number of columns.
    pub cols: u16,
}

impl PaneSize {
    /// Create a new pane size.
    #[must_use]
    pub fn new(rows: u16, cols: u16) -> Self {
        Self { rows, cols }
    }
}

/// Configuration for spawning a new pane.
#[derive(Clone, Debug, Default)]
pub struct SpawnConfig {
    /// Command to run. If None, uses the default shell.
    pub command: Option<String>,

    /// Arguments to pass to the command.
    pub args: Vec<String>,

    /// Initial size of the pane.
    pub size: PaneSize,

    /// Working directory for the process.
    pub cwd: Option<PathBuf>,

    /// Additional environment variables.
    pub env: HashMap<String, String>,

    /// Scrollback buffer size in lines.
    pub scrollback: usize,
}

impl SpawnConfig {
    /// Create a new spawn config with default shell.
    #[must_use]
    pub fn new(size: PaneSize) -> Self {
        Self {
            size,
            scrollback: 10_000,
            ..Default::default()
        }
    }

    /// Set the command to run.
    #[must_use]
    pub fn command(mut self, cmd: impl Into<String>) -> Self {
        self.command = Some(cmd.into());
        self
    }

    /// Set command arguments.
    #[must_use]
    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Set the working directory.
    #[must_use]
    pub fn cwd(mut self, path: impl Into<PathBuf>) -> Self {
        self.cwd = Some(path.into());
        self
    }

    /// Add an environment variable.
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set the scrollback buffer size.
    #[must_use]
    pub fn scrollback(mut self, lines: usize) -> Self {
        self.scrollback = lines;
        self
    }
}

/// A snapshot of the terminal screen state.
#[derive(Clone, Debug)]
pub struct ScreenSnapshot {
    /// Screen content as a 2D grid of cells.
    cells: Vec<Vec<ScreenCell>>,
    /// Cursor position (row, col).
    cursor: (u16, u16),
    /// Screen size.
    size: PaneSize,
}

/// A single cell in the terminal screen.
#[derive(Clone, Debug, Default)]
pub struct ScreenCell {
    /// The character in this cell.
    pub char: char,
    /// Foreground color.
    pub fg: ScreenColor,
    /// Background color.
    pub bg: ScreenColor,
    /// Text is bold.
    pub bold: bool,
    /// Text is italic.
    pub italic: bool,
    /// Text is underlined.
    pub underline: bool,
    /// Text is inverse (swapped fg/bg).
    pub inverse: bool,
}

/// Terminal color representation.
#[derive(Clone, Copy, Debug, Default)]
pub enum ScreenColor {
    /// Default terminal color.
    #[default]
    Default,
    /// Indexed color (0-255).
    Indexed(u8),
    /// RGB color.
    Rgb(u8, u8, u8),
}

impl ScreenSnapshot {
    /// Create a snapshot from a vt100 parser.
    pub(crate) fn from_parser(parser: &vt100::Parser) -> Self {
        let screen = parser.screen();
        let size = PaneSize::new(screen.size().0, screen.size().1);
        let (cursor_row, cursor_col) = screen.cursor_position();

        let mut cells = Vec::with_capacity(size.rows as usize);
        for row in 0..size.rows {
            let mut row_cells = Vec::with_capacity(size.cols as usize);
            for col in 0..size.cols {
                let cell = screen
                    .cell(row, col)
                    .map_or_else(ScreenCell::default, |c| ScreenCell {
                        char: c.contents().chars().next().unwrap_or(' '),
                        fg: convert_vt100_color(c.fgcolor()),
                        bg: convert_vt100_color(c.bgcolor()),
                        bold: c.bold(),
                        italic: c.italic(),
                        underline: c.underline(),
                        inverse: c.inverse(),
                    });
                row_cells.push(cell);
            }
            cells.push(row_cells);
        }

        Self {
            cells,
            cursor: (cursor_row, cursor_col),
            size,
        }
    }

    /// Get the screen size.
    #[must_use]
    pub fn size(&self) -> PaneSize {
        self.size
    }

    /// Get the cursor position (row, col).
    #[must_use]
    pub fn cursor(&self) -> (u16, u16) {
        self.cursor
    }

    /// Get a cell at the given position.
    #[must_use]
    pub fn cell(&self, row: u16, col: u16) -> Option<&ScreenCell> {
        self.cells
            .get(row as usize)
            .and_then(|r| r.get(col as usize))
    }

    /// Iterate over all rows.
    pub fn rows(&self) -> impl Iterator<Item = &[ScreenCell]> {
        self.cells.iter().map(Vec::as_slice)
    }
}

fn convert_vt100_color(color: vt100::Color) -> ScreenColor {
    match color {
        vt100::Color::Default => ScreenColor::Default,
        vt100::Color::Idx(idx) => ScreenColor::Indexed(idx),
        vt100::Color::Rgb(r, g, b) => ScreenColor::Rgb(r, g, b),
    }
}

/// Public handle for controlling a pane.
///
/// This handle can be cloned and shared across threads.
#[derive(Clone)]
pub struct PaneHandle {
    /// Pane ID.
    id: PaneId,

    /// Channel to send input to the pane.
    input_tx: mpsc::Sender<Vec<u8>>,

    /// Watch channel for state changes.
    state_rx: watch::Receiver<PaneState>,

    /// Shared screen state for reading.
    screen: Arc<RwLock<vt100::Parser>>,

    /// Pane title.
    title: Arc<RwLock<String>>,
}

impl PaneHandle {
    /// Create a new pane handle.
    pub(crate) fn new(
        id: PaneId,
        input_tx: mpsc::Sender<Vec<u8>>,
        state_rx: watch::Receiver<PaneState>,
        screen: Arc<RwLock<vt100::Parser>>,
    ) -> Self {
        Self {
            id,
            input_tx,
            state_rx,
            screen,
            title: Arc::new(RwLock::new(String::new())),
        }
    }

    /// Get the pane ID.
    #[must_use]
    pub fn id(&self) -> PaneId {
        self.id
    }

    /// Send input bytes to the pane's PTY.
    ///
    /// # Errors
    /// Returns an error if the pane has been closed.
    pub async fn send_input(&self, data: &[u8]) -> Result<()> {
        self.input_tx
            .send(data.to_vec())
            .await
            .map_err(|_| Error::PaneClosed)
    }

    /// Get the current pane state.
    #[must_use]
    pub fn state(&self) -> PaneState {
        self.state_rx.borrow().clone()
    }

    /// Check if the pane's process is still alive.
    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.state().is_alive()
    }

    /// Get a snapshot of the terminal screen.
    #[must_use]
    pub fn screen_snapshot(&self) -> ScreenSnapshot {
        let screen = self.screen.read().expect("screen lock poisoned");
        ScreenSnapshot::from_parser(&screen)
    }

    /// Get direct access to the screen parser for widget rendering.
    pub(crate) fn screen(&self) -> &Arc<RwLock<vt100::Parser>> {
        &self.screen
    }

    /// Get the pane title.
    #[must_use]
    pub fn title(&self) -> String {
        self.title.read().expect("title lock poisoned").clone()
    }

    /// Set the pane title.
    #[allow(dead_code)]
    pub(crate) fn set_title(&self, title: String) {
        *self.title.write().expect("title lock poisoned") = title;
    }
}

impl std::fmt::Debug for PaneHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneHandle")
            .field("id", &self.id)
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}
