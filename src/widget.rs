//! Ratatui widgets for rendering panes.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};

use crate::pane::{PaneHandle, PaneId, ScreenColor};

/// Widget for rendering a single pane's terminal content.
pub struct PaneWidget<'a> {
    /// The pane handle to render.
    handle: &'a PaneHandle,
    /// Whether this pane is focused.
    focused: bool,
    /// Border block.
    block: Option<Block<'a>>,
    /// Style for focused state.
    focus_style: Style,
    /// Show cursor.
    show_cursor: bool,
}

impl<'a> PaneWidget<'a> {
    /// Create a new pane widget.
    #[must_use]
    pub fn new(handle: &'a PaneHandle) -> Self {
        Self {
            handle,
            focused: false,
            block: None,
            focus_style: Style::default().fg(Color::Cyan),
            show_cursor: true,
        }
    }

    /// Set whether this pane is focused.
    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set the block (border).
    #[must_use]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Set the focus style.
    #[must_use]
    pub fn focus_style(mut self, style: Style) -> Self {
        self.focus_style = style;
        self
    }

    /// Set whether to show the cursor.
    #[must_use]
    pub fn show_cursor(mut self, show: bool) -> Self {
        self.show_cursor = show;
        self
    }

    /// Create a default block for the pane.
    fn default_block(&self, title: &str) -> Block<'a> {
        let style = if self.focused {
            self.focus_style
        } else {
            Style::default().fg(Color::DarkGray)
        };

        Block::default()
            .borders(Borders::ALL)
            .border_style(style)
            .title(title.to_string())
    }
}

impl Widget for PaneWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Get screen state synchronously
        let screen = self.handle.screen().read().expect("screen lock poisoned");
        let vt_screen = screen.screen();

        // Determine the block to use
        let pane_title = format!("Pane {}", self.handle.id());
        let block = match self.block {
            Some(b) => b,
            None => self.default_block(&pane_title),
        };

        // Render block and get inner area
        let inner_area = block.inner(area);
        block.render(area, buf);

        // Render terminal content
        let (cursor_row, cursor_col) = vt_screen.cursor_position();

        for row in 0..inner_area.height {
            for col in 0..inner_area.width {
                let x = inner_area.x + col;
                let y = inner_area.y + row;

                if x >= buf.area.x + buf.area.width || y >= buf.area.y + buf.area.height {
                    continue;
                }

                if let Some(cell) = vt_screen.cell(row, col) {
                    let buf_cell = &mut buf[(x, y)];

                    // Set character
                    let ch = cell.contents().chars().next().unwrap_or(' ');
                    buf_cell.set_char(ch);

                    // Convert colors
                    let mut fg = convert_color(cell.fgcolor());
                    let mut bg = convert_color(cell.bgcolor());

                    // Handle inverse
                    if cell.inverse() {
                        std::mem::swap(&mut fg, &mut bg);
                    }

                    buf_cell.set_fg(fg);
                    buf_cell.set_bg(bg);

                    // Apply modifiers
                    let mut style = Style::default();
                    if cell.bold() {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    if cell.italic() {
                        style = style.add_modifier(Modifier::ITALIC);
                    }
                    if cell.underline() {
                        style = style.add_modifier(Modifier::UNDERLINED);
                    }
                    buf_cell.set_style(style);
                }
            }
        }

        // Render cursor if focused and visible
        if self.focused && self.show_cursor {
            let cursor_x = inner_area.x + cursor_col;
            let cursor_y = inner_area.y + cursor_row;

            if cursor_x < inner_area.x + inner_area.width
                && cursor_y < inner_area.y + inner_area.height
                && cursor_x < buf.area.x + buf.area.width
                && cursor_y < buf.area.y + buf.area.height
            {
                let cell = &mut buf[(cursor_x, cursor_y)];
                cell.set_style(Style::default().add_modifier(Modifier::REVERSED));
            }
        }
    }
}

/// Widget for rendering the entire multiplexer.
pub struct CockpitWidget<'a> {
    /// Pane handles by ID.
    panes: &'a [(PaneId, &'a PaneHandle)],
    /// Layout areas by pane ID.
    areas: &'a [(PaneId, Rect)],
    /// Currently focused pane.
    focused: Option<PaneId>,
    /// Style for focused pane border.
    focus_style: Style,
    /// Style for unfocused pane borders.
    unfocus_style: Style,
}

impl<'a> CockpitWidget<'a> {
    /// Create a new cockpit widget.
    #[must_use]
    pub fn new(
        panes: &'a [(PaneId, &'a PaneHandle)],
        areas: &'a [(PaneId, Rect)],
        focused: Option<PaneId>,
    ) -> Self {
        Self {
            panes,
            areas,
            focused,
            focus_style: Style::default().fg(Color::Cyan),
            unfocus_style: Style::default().fg(Color::DarkGray),
        }
    }

    /// Set the focus style.
    #[must_use]
    pub fn focus_style(mut self, style: Style) -> Self {
        self.focus_style = style;
        self
    }

    /// Set the unfocus style.
    #[must_use]
    pub fn unfocus_style(mut self, style: Style) -> Self {
        self.unfocus_style = style;
        self
    }
}

impl Widget for CockpitWidget<'_> {
    fn render(self, _area: Rect, buf: &mut Buffer) {
        // Create a lookup for pane handles
        let pane_map: std::collections::HashMap<_, _> =
            self.panes.iter().map(|(id, h)| (*id, *h)).collect();

        // Render each pane in its area
        for (pane_id, pane_area) in self.areas {
            if let Some(handle) = pane_map.get(pane_id) {
                let is_focused = self.focused == Some(*pane_id);
                let border_style = if is_focused {
                    self.focus_style
                } else {
                    self.unfocus_style
                };

                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(format!("Pane {pane_id}"));

                let widget = PaneWidget::new(handle)
                    .focused(is_focused)
                    .block(block)
                    .focus_style(self.focus_style);

                widget.render(*pane_area, buf);
            }
        }
    }
}

/// Convert a vt100 color to a ratatui color.
fn convert_color(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(idx) => Color::Indexed(idx),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

/// Convert a screen color to a ratatui color.
#[allow(dead_code)]
fn convert_screen_color(color: ScreenColor) -> Color {
    match color {
        ScreenColor::Default => Color::Reset,
        ScreenColor::Indexed(idx) => Color::Indexed(idx),
        ScreenColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}
