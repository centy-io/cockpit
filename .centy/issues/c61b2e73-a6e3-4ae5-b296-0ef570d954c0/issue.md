# feat: Add batteries-included high-level API

## Summary

Cockpit currently provides low-level building blocks for terminal multiplexing. Developers must manually handle:

- Terminal setup/teardown (~15 lines of boilerplate)
- Event loop (~100+ lines)
- Layout construction
- Keybinding handling
- Resize events
- Process spawning with SpawnConfig

**Goal**: Add a high-level "batteries-included" API where developers just specify processes to run, and cockpit handles everything else.

## Current State (Building Blocks)

```rust
// Developer must handle everything manually:
enable_raw_mode()?;
execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
let mut terminal = Terminal::new(backend)?;

let mut manager = PaneManager::new();
let pane1 = manager.spawn(SpawnConfig::new(size))?;
let pane2 = manager.spawn(SpawnConfig::new(size))?;
manager.set_layout(Layout::vsplit_equal(...));

loop {
    terminal.draw(|frame| { /* render */ })?;
    if event::poll(...)? { /* handle keys, mouse */ }
}

disable_raw_mode()?;
```

## Proposed API

### Simple Case
```rust
Cockpit::run(&["npm run dev", "cargo watch -x check"])
```

### Builder Pattern
```rust
Cockpit::builder()
    .process("npm run dev").name("frontend")
    .process("cargo watch").name("backend").cwd("./server")
    .layout(LayoutKind::VSplit)  // or Auto, HSplit, Grid
    .on_exit(ExitBehavior::ShowMessage)
    .run()
```

## Components Needed

### 1. CockpitApp (Core Runner)
- RAII terminal setup/teardown
- Built-in event loop with default keybindings
- Automatic terminal resize handling
- Graceful shutdown

### 2. ProcessSpec (Simplified Config)
```rust
pub struct ProcessSpec {
    pub command: String,       // Parsed into program + args
    pub name: Option<String>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
}
```

### 3. LayoutKind (High-Level Layouts)
```rust
pub enum LayoutKind {
    Auto,    // Choose based on pane count
    VSplit,  // Side-by-side
    HSplit,  // Stacked
    Grid,    // 2x2, 2x3, etc.
}
```

### 4. ExitBehavior
```rust
pub enum ExitBehavior {
    Keep,           // Keep pane with exit message
    Remove,         // Remove from layout
    Restart,        // Restart process
    RestartOnError, // Restart on non-zero exit
}
```

### 5. Default Keybindings
| Key | Action |
|-----|--------|
| Ctrl+Q | Quit |
| Ctrl+C (2x) | Exit dialog |
| Ctrl+N / Ctrl+P | Focus next/prev |
| Mouse click | Focus pane |

## Edge Cases

- Process fails to spawn → log error, continue with others
- Focused pane exits → auto-focus next
- All panes exit → quit app (configurable)
- Terminal resize → auto-resize all panes

## Implementation Phases

1. **Core Runner** (~300 lines): CockpitApp, event loop, defaults
2. **Builder Pattern** (~200 lines): CockpitBuilder, ProcessSpec, LayoutKind
3. **Advanced Features** (~200 lines): ExitBehavior, hooks, custom keybinds
