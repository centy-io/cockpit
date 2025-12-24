//! PTY spawning and I/O management.

use std::io::{Read, Write};
use std::sync::{Arc, RwLock};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::error::{Error, Result};
use crate::pane::{PaneHandle, PaneId, PaneSize, PaneState, SpawnConfig};

/// Events emitted by panes.
#[derive(Clone, Debug)]
pub enum PaneEvent {
    /// Process exited normally.
    Exited { pane_id: PaneId, code: i32 },

    /// Process crashed or was killed.
    Crashed {
        pane_id: PaneId,
        signal: Option<i32>,
        error: String,
    },

    /// Title changed (via OSC escape sequence).
    TitleChanged { pane_id: PaneId, title: String },

    /// Output received (for debugging).
    Output { pane_id: PaneId, size: usize },
}

/// Result of spawning a PTY process.
pub(crate) struct SpawnedPty {
    /// Handle for controlling the pane.
    pub handle: PaneHandle,

    /// PTY master for resize operations.
    pub pty_master: Box<dyn portable_pty::MasterPty + Send>,

    /// Handle to the reader task.
    pub reader_handle: JoinHandle<()>,

    /// Handle to the writer task.
    pub writer_handle: JoinHandle<()>,

    /// Handle to the process monitor task.
    pub monitor_handle: JoinHandle<()>,
}

/// Spawns a new PTY process.
///
/// # Errors
/// Returns an error if PTY creation or process spawning fails.
pub(crate) fn spawn_pty(
    pane_id: PaneId,
    config: &SpawnConfig,
    event_tx: mpsc::Sender<PaneEvent>,
) -> Result<SpawnedPty> {
    let pty_system = native_pty_system();

    // Create PTY pair
    let pty_pair = pty_system
        .openpty(PtySize {
            rows: config.size.rows,
            cols: config.size.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| Error::PtyCreate(e.to_string()))?;

    // Build command
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let mut cmd = match &config.command {
        Some(c) => {
            let mut builder = CommandBuilder::new(c);
            for arg in &config.args {
                builder.arg(arg);
            }
            builder
        }
        None => CommandBuilder::new(&shell),
    };

    // Set working directory
    if let Some(cwd) = &config.cwd {
        cmd.cwd(cwd);
    }

    // Set environment variables
    for (key, value) in &config.env {
        cmd.env(key, value);
    }

    // Spawn the child process
    let child = pty_pair.slave.spawn_command(cmd)?;

    // Create vt100 parser for terminal emulation
    let parser = vt100::Parser::new(config.size.rows, config.size.cols, config.scrollback);
    let screen = Arc::new(RwLock::new(parser));

    // Create channels
    let (input_tx, input_rx) = mpsc::channel::<Vec<u8>>(256);
    let (state_tx, state_rx) = watch::channel(PaneState::Running);

    // Spawn reader task
    let reader_handle = spawn_reader_task(
        pane_id,
        pty_pair.master.try_clone_reader()?,
        screen.clone(),
        event_tx.clone(),
    );

    // Spawn writer task
    let writer_handle = spawn_writer_task(pty_pair.master.take_writer()?, input_rx);

    // Spawn process monitor task
    let monitor_handle = spawn_monitor_task(pane_id, child, state_tx, event_tx);

    // Create pane handle
    let handle = PaneHandle::new(pane_id, input_tx, state_rx, screen);

    Ok(SpawnedPty {
        handle,
        pty_master: pty_pair.master,
        reader_handle,
        writer_handle,
        monitor_handle,
    })
}

/// Resize a PTY.
///
/// # Errors
/// Returns an error if the resize operation fails.
pub(crate) fn resize_pty(pty_master: &dyn portable_pty::MasterPty, size: PaneSize) -> Result<()> {
    pty_master
        .resize(PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| Error::Resize(e.to_string()))
}

/// Spawns the task that reads PTY output.
fn spawn_reader_task(
    pane_id: PaneId,
    mut reader: Box<dyn Read + Send>,
    screen: Arc<RwLock<vt100::Parser>>,
    event_tx: mpsc::Sender<PaneEvent>,
) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 4096];

        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    // EOF - process has closed
                    break;
                }
                Ok(n) => {
                    let data = &buf[..n];

                    // Update screen state
                    {
                        let mut screen = screen.write().expect("screen lock poisoned");
                        screen.process(data);
                    }

                    // Emit output event (optional, for debugging)
                    let _ = event_tx.blocking_send(PaneEvent::Output { pane_id, size: n });
                }
                Err(e) => {
                    tracing::debug!("PTY read error for pane {}: {}", pane_id, e);
                    break;
                }
            }
        }

        tracing::debug!("Reader task for pane {} finished", pane_id);
    })
}

/// Spawns the task that writes to PTY.
fn spawn_writer_task(
    mut writer: Box<dyn Write + Send>,
    mut input_rx: mpsc::Receiver<Vec<u8>>,
) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        while let Some(data) = input_rx.blocking_recv() {
            if let Err(e) = writer.write_all(&data) {
                tracing::debug!("PTY write error: {}", e);
                break;
            }
            if let Err(e) = writer.flush() {
                tracing::debug!("PTY flush error: {}", e);
                break;
            }
        }

        tracing::debug!("Writer task finished");
    })
}

/// Spawns the task that monitors process exit.
fn spawn_monitor_task(
    pane_id: PaneId,
    mut child: Box<dyn portable_pty::Child + Send>,
    state_tx: watch::Sender<PaneState>,
    event_tx: mpsc::Sender<PaneEvent>,
) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        match child.wait() {
            Ok(status) => {
                let code = status.exit_code() as i32;
                if status.success() {
                    let new_state = PaneState::Exited { code };
                    let _ = state_tx.send(new_state);
                    let _ = event_tx.blocking_send(PaneEvent::Exited { pane_id, code });
                } else {
                    // Non-zero exit - could be error or signal
                    // portable-pty doesn't expose signal info directly
                    let new_state = PaneState::Exited { code };
                    let _ = state_tx.send(new_state);
                    let _ = event_tx.blocking_send(PaneEvent::Exited { pane_id, code });
                }
            }
            Err(e) => {
                let new_state = PaneState::Crashed {
                    signal: None,
                    error: Some(e.to_string()),
                };
                let _ = state_tx.send(new_state);
                let _ = event_tx.blocking_send(PaneEvent::Crashed {
                    pane_id,
                    signal: None,
                    error: e.to_string(),
                });
            }
        }

        tracing::debug!("Monitor task for pane {} finished", pane_id);
    })
}
