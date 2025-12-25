//! Basic example demonstrating cockpit's terminal multiplexer.
//!
//! Run with: cargo run --example basic
//!
//! Spawns 4 bash terminals in the upper panes.
//!
//! Controls:
//! - Ctrl+C (twice): Open exit confirmation dialog
//! - Ctrl+Q: Quit immediately
//! - Ctrl+N: Focus next pane
//! - Mouse click: Focus pane under cursor
//! - All other input goes to the focused pane
//!
//! Layout:
//! - 4 PTY panes on top
//! - 8 sub-panes below

use std::io::{self, stdout};
use std::time::{Duration, Instant};

use cockpit::{
    CockpitWidget, ConfirmDialog, DialogState, GitUserPlugin, PaneManager, SpawnConfig,
    StatusBarWidget, STATUS_BAR_HEIGHT,
};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, layout::Rect, Terminal};

#[tokio::main]
async fn main() -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app
    let result = run_app(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {e}");
    }

    Ok(())
}

/// Time window for detecting double Ctrl+C press (500ms).
const CTRL_C_WINDOW: Duration = Duration::from_millis(500);

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> cockpit::Result<()> {
    // Create pane manager with plugin support
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut manager = PaneManager::new().with_plugins(cwd);

    // Register the git user plugin for the status bar
    let _ = manager.register_plugin(Box::new(GitUserPlugin::new()));

    // Get terminal size and set it in the manager
    let term_size = terminal.size()?;
    let panes_area = Rect {
        x: 0,
        y: STATUS_BAR_HEIGHT,
        width: term_size.width,
        height: term_size.height.saturating_sub(STATUS_BAR_HEIGHT),
    };

    manager.set_terminal_size(panes_area);

    // Spawn four bash panes
    manager.spawn(SpawnConfig::new_shell())?;
    manager.spawn(SpawnConfig::new_shell())?;
    manager.spawn(SpawnConfig::new_shell())?;
    manager.spawn(SpawnConfig::new_shell())?;

    // Dialog state for exit confirmation
    let mut dialog_state = DialogState::new();
    let mut dialog_area = ratatui::layout::Rect::default();

    // Track Ctrl+C press timing for double-press detection
    let mut last_ctrl_c: Option<Instant> = None;

    // Main event loop
    loop {
        // Draw UI
        terminal.draw(|frame| {
            let area = frame.area();

            // Reserve space for status bar at the top
            let status_bar_area = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: STATUS_BAR_HEIGHT,
            };
            let panes_area = Rect {
                x: area.x,
                y: area.y + STATUS_BAR_HEIGHT,
                width: area.width,
                height: area.height.saturating_sub(STATUS_BAR_HEIGHT),
            };

            // Render status bar with plugin segments
            let segments = manager.status_bar_segments();
            let status_bar = StatusBarWidget::new(&segments);
            frame.render_widget(status_bar, status_bar_area);

            // Get pre-calculated layout areas (automatic!)
            let areas = manager.get_areas();
            let areas_vec: Vec<_> = areas.iter().map(|(&id, &rect)| (id, rect)).collect();

            // Get pane handles
            let pane_ids = manager.pane_ids();
            let panes: Vec<_> = pane_ids
                .iter()
                .filter_map(|id| manager.get_pane(*id).map(|h| (*id, h)))
                .collect();

            // Build the widget
            let sub_panes = manager.get_sub_pane_areas();
            let empty_panes = manager.get_empty_pane_areas();
            let widget = CockpitWidget::new(&panes, &areas_vec, manager.focused())
                .sub_panes(sub_panes)
                .empty_panes(empty_panes);

            frame.render_widget(widget, panes_area);

            // Render exit confirmation dialog if visible
            if dialog_state.visible {
                dialog_area = DialogState::calculate_area(area);
                let dialog =
                    ConfirmDialog::new(" Exit Cockpit? ", "Are you sure you want to quit?")
                        .selected(dialog_state.selected);
                frame.render_widget(dialog, dialog_area);
            }
        })?;

        // Handle events with a short timeout for responsive updates
        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => {
                    // If dialog is visible, route input to dialog
                    if dialog_state.visible {
                        if let Some(confirmed) = dialog_state.handle_key(key) {
                            if confirmed {
                                break; // User confirmed exit
                            }
                            // User cancelled, continue running
                        }
                        continue;
                    }

                    // Check for quit (Ctrl+Q) - immediate exit without dialog
                    if key.code == KeyCode::Char('q')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break;
                    }

                    // Check for Ctrl+C double-press to show exit dialog
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        let now = Instant::now();
                        if let Some(last) = last_ctrl_c {
                            if now.duration_since(last) < CTRL_C_WINDOW {
                                // Double Ctrl+C detected, show dialog
                                dialog_state.show();
                                last_ctrl_c = None;
                                continue;
                            }
                        }
                        // First Ctrl+C, record time and send to pane
                        last_ctrl_c = Some(now);
                        manager.route_key(key).await?;
                        continue;
                    }

                    // Reset Ctrl+C tracking on any other key
                    last_ctrl_c = None;

                    // Check for focus switch (Ctrl+N)
                    if key.code == KeyCode::Char('n')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        manager.focus_next();
                        continue;
                    }

                    // Route all other input to focused pane
                    manager.route_key(key).await?;
                }
                Event::Resize(width, height) => {
                    // Recalculate layout on terminal resize
                    let panes_area = Rect {
                        x: 0,
                        y: STATUS_BAR_HEIGHT,
                        width,
                        height: height.saturating_sub(STATUS_BAR_HEIGHT),
                    };
                    manager.set_terminal_size(panes_area);
                }
                Event::Mouse(mouse) => {
                    // If dialog is visible, handle mouse for dialog
                    if dialog_state.visible {
                        if matches!(mouse.kind, MouseEventKind::Down(_)) {
                            if let Some(confirmed) =
                                dialog_state.handle_mouse(mouse.column, mouse.row, dialog_area)
                            {
                                if confirmed {
                                    break; // User clicked Yes
                                }
                                // User clicked No, continue running
                            }
                        }
                        continue;
                    }

                    // Handle mouse click to switch focus
                    if matches!(mouse.kind, MouseEventKind::Down(_)) {
                        let areas = manager.get_areas().clone();
                        manager.focus_at_position(mouse.column, mouse.row, &areas);
                    }
                }
                _ => {}
            }
        }

        // Tick plugins to refresh status bar data
        manager.tick_plugins();

        // Poll for pane events (crashes, exits, etc.)
        let events = manager.poll_events();
        for event in events {
            match event {
                cockpit::PaneEvent::Exited { pane_id, code } => {
                    eprintln!("Pane {pane_id} exited with code {code}");
                }
                cockpit::PaneEvent::Crashed {
                    pane_id,
                    signal,
                    error,
                } => {
                    eprintln!("Pane {pane_id} crashed: {error} (signal: {signal:?})");
                }
                _ => {}
            }
        }
    }

    Ok(())
}
