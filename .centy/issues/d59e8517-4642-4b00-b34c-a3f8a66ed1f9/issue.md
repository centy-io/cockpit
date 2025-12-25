# Move panel arrangement from tui-manager to cockpit

## Summary

Move the panel arrangement and sizing logic from tui-manager to cockpit. The tui-manager should simply request "open 2 instances of centy" and cockpit will handle all layout/sizing.

## Current Behavior

Currently, tui-manager handles:
- Calculating pane sizes (pane_width = term_size.width / 2)
- Creating layout structure (Layout::vsplit_equal())
- Setting the layout on the manager
- Calculating areas for rendering

## Proposed Behavior

Cockpit should handle all panel arrangement internally. The tui-manager should just:
1. Tell cockpit to spawn 2 centy instances
2. Cockpit auto-arranges them (side-by-side by default)
3. Cockpit handles all sizing and resize events

## Benefits

- Separation of concerns: Cockpit owns layout, tui-manager owns application logic
- Simpler consumer API: Less boilerplate for applications using cockpit
- Consistent behavior: Layout logic centralized in one place
- Easier to extend: Adding new layout modes only requires cockpit changes

## Tasks

- [ ] Add auto-layout capability to cockpit PaneManager
- [ ] Remove layout/sizing logic from tui-manager
- [ ] Simplify tui-manager to just spawn commands
- [ ] Ensure resize events are handled entirely within cockpit
