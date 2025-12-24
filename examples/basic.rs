//! Basic example demonstrating cockpit's terminal multiplexer functionality.
//!
//! Run with: cargo run --example basic
//!
//! Controls:
//! - Ctrl+C (twice): Open exit confirmation dialog
//! - Ctrl+Q: Quit immediately
//! - Ctrl+N: Focus next pane
//! - Mouse click: Focus pane under cursor
//! - All other input goes to the focused pane

use std::io::{self, stdout};
use std::time::{Duration, Instant};

use cockpit::{CockpitWidget, ConfirmDialog, DialogState, Layout, PaneManager, PaneSize, SpawnConfig};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

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
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {e}");
    }

    Ok(())
}

/// Time window for detecting double Ctrl+C press (500ms).
const CTRL_C_WINDOW: Duration = Duration::from_millis(500);

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> cockpit::Result<()> {
    // Create pane manager
    let mut manager = PaneManager::new();

    // Get terminal size
    let term_size = terminal.size()?;
    let pane_size = PaneSize::new(term_size.height / 2, term_size.width / 2);

    // Spawn two panes with shells
    let pane1 = manager.spawn(SpawnConfig::new(pane_size))?;
    let pane2 = manager.spawn(SpawnConfig::new(pane_size))?;

    // Set up a vertical split layout (side by side)
    let layout = Layout::vsplit_equal(Layout::single(pane1.id()), Layout::single(pane2.id()));
    manager.set_layout(layout);

    // Track current areas for mouse handling
    let mut current_areas = std::collections::HashMap::new();

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

            // Calculate layout areas
            let areas = manager.calculate_areas(area);
            current_areas.clone_from(&areas);
            let areas_vec: Vec<_> = areas.into_iter().collect();

            // Get pane handles
            let pane_ids = manager.pane_ids();
            let panes: Vec<_> = pane_ids
                .iter()
                .filter_map(|id| manager.get_pane(*id).map(|h| (*id, h)))
                .collect();

            // Render the cockpit widget
            let widget = CockpitWidget::new(&panes, &areas_vec, manager.focused());
            frame.render_widget(widget, area);

            // Render exit confirmation dialog if visible
            if dialog_state.visible {
                dialog_area = DialogState::calculate_area(area);
                let dialog = ConfirmDialog::new(" Exit Cockpit? ", "Are you sure you want to quit?")
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
                    if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break;
                    }

                    // Check for Ctrl+C double-press to show exit dialog
                    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
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
                    if key.code == KeyCode::Char('n') && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        manager.focus_next();
                        continue;
                    }

                    // Route all other input to focused pane
                    manager.route_key(key).await?;
                }
                Event::Mouse(mouse) => {
                    // If dialog is visible, handle mouse for dialog
                    if dialog_state.visible {
                        if matches!(mouse.kind, MouseEventKind::Down(_)) {
                            if let Some(confirmed) = dialog_state.handle_mouse(mouse.column, mouse.row, dialog_area) {
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
                        manager.focus_at_position(mouse.column, mouse.row, &current_areas);
                    }
                }
                _ => {}
            }
        }

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
