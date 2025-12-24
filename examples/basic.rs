//! Basic example demonstrating cockpit's terminal multiplexer functionality.
//!
//! Run with: cargo run --example basic
//!
//! Controls:
//! - Ctrl+Q: Quit
//! - Ctrl+N: Focus next pane
//! - Mouse click: Focus pane under cursor
//! - All other input goes to the focused pane

use std::io::{self, stdout};
use std::time::Duration;

use cockpit::{CockpitWidget, Layout, PaneManager, PaneSize, SpawnConfig};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind,
    },
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
        })?;

        // Handle events with a short timeout for responsive updates
        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => {
                    // Check for quit (Ctrl+Q)
                    if key.code == KeyCode::Char('q')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break;
                    }

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
                Event::Mouse(mouse) => {
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
