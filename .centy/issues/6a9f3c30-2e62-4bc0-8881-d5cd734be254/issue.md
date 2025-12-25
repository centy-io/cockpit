# Adjust layout on terminal resize

The application should dynamically adjust the pane layout when the terminal window is resized. Currently, the layout is calculated once at startup but does not respond to resize events.

## Expected Behavior
- When the terminal is resized, all panes should recalculate their areas
- The 70/30 split ratio between upper panes and sub-panes should be maintained
- PTY sizes should be updated to match new pane dimensions
- The UI should re-render smoothly without visual artifacts

## Implementation Notes
- Handle `Event::Resize` in the main event loop
- Call `manager.set_terminal_size()` with the new dimensions
- Ensure PTY resize signals are sent to child processes
