//! Ratatui widgets for rendering panes.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::pane::{PaneHandle, PaneId, ScreenColor};

/// Which button is selected in a confirm dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogButton {
    /// The "Yes" button is selected.
    #[default]
    Yes,
    /// The "No" button is selected.
    No,
}

impl DialogButton {
    /// Toggle to the other button.
    #[must_use]
    pub fn toggle(self) -> Self {
        match self {
            Self::Yes => Self::No,
            Self::No => Self::Yes,
        }
    }
}

/// State for the confirm dialog.
#[derive(Debug, Clone, Default)]
pub struct DialogState {
    /// Whether the dialog is visible.
    pub visible: bool,
    /// Which button is currently selected.
    pub selected: DialogButton,
}

impl DialogState {
    /// Create a new hidden dialog state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Show the dialog with Yes selected by default.
    pub fn show(&mut self) {
        self.visible = true;
        self.selected = DialogButton::Yes;
    }

    /// Hide the dialog.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Select the next button (toggle between Yes and No).
    pub fn next(&mut self) {
        self.selected = self.selected.toggle();
    }

    /// Select the previous button (toggle between Yes and No).
    pub fn prev(&mut self) {
        self.selected = self.selected.toggle();
    }

    /// Handle a key press. Returns Some(true) for Yes, Some(false) for No, None if not handled.
    #[must_use]
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Option<bool> {
        use crossterm::event::KeyCode;

        if !self.visible {
            return None;
        }

        match key.code {
            KeyCode::Char('y' | 'Y') => {
                self.hide();
                Some(true)
            }
            KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                self.hide();
                Some(false)
            }
            KeyCode::Enter => {
                let result = self.selected == DialogButton::Yes;
                self.hide();
                Some(result)
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
                self.next();
                None
            }
            KeyCode::Up | KeyCode::Down => {
                // Arrow keys also toggle
                self.next();
                None
            }
            _ => None,
        }
    }

    /// Handle a mouse click. Returns Some(true) for Yes, Some(false) for No, None if not on a button.
    #[must_use]
    pub fn handle_mouse(&mut self, x: u16, y: u16, dialog_area: Rect) -> Option<bool> {
        if !self.visible {
            return None;
        }

        // Calculate button areas (matching ConfirmDialog rendering)
        let button_y = dialog_area.y + dialog_area.height - 3;
        let yes_x_start = dialog_area.x + dialog_area.width / 2 - 8;
        let yes_x_end = yes_x_start + 6;
        let no_x_start = dialog_area.x + dialog_area.width / 2 + 2;
        let no_x_end = no_x_start + 5;

        if y == button_y {
            if x >= yes_x_start && x < yes_x_end {
                self.hide();
                return Some(true);
            }
            if x >= no_x_start && x < no_x_end {
                self.hide();
                return Some(false);
            }
        }

        None
    }

    /// Calculate the dialog area for a given terminal size.
    #[must_use]
    pub fn calculate_area(terminal_area: Rect) -> Rect {
        let width = 40.min(terminal_area.width.saturating_sub(4));
        let height = 7.min(terminal_area.height.saturating_sub(2));
        let x = terminal_area.x + (terminal_area.width.saturating_sub(width)) / 2;
        let y = terminal_area.y + (terminal_area.height.saturating_sub(height)) / 2;
        Rect::new(x, y, width, height)
    }
}

/// A confirmation dialog widget.
pub struct ConfirmDialog<'a> {
    /// Title of the dialog.
    title: &'a str,
    /// Message to display.
    message: &'a str,
    /// Which button is selected.
    selected: DialogButton,
    /// Style for the dialog border.
    border_style: Style,
    /// Style for the selected button.
    selected_style: Style,
    /// Style for the unselected button.
    unselected_style: Style,
}

impl<'a> ConfirmDialog<'a> {
    /// Create a new confirm dialog.
    #[must_use]
    pub fn new(title: &'a str, message: &'a str) -> Self {
        Self {
            title,
            message,
            selected: DialogButton::Yes,
            border_style: Style::default().fg(Color::Yellow),
            selected_style: Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD),
            unselected_style: Style::default().fg(Color::White),
        }
    }

    /// Set which button is selected.
    #[must_use]
    pub fn selected(mut self, selected: DialogButton) -> Self {
        self.selected = selected;
        self
    }

    /// Set the border style.
    #[must_use]
    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }

    /// Set the selected button style.
    #[must_use]
    pub fn selected_style(mut self, style: Style) -> Self {
        self.selected_style = style;
        self
    }

    /// Set the unselected button style.
    #[must_use]
    pub fn unselected_style(mut self, style: Style) -> Self {
        self.unselected_style = style;
        self
    }
}

impl Widget for ConfirmDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the area behind the dialog
        Clear.render(area, buf);

        // Create dialog block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.border_style)
            .title(self.title);

        let inner = block.inner(area);
        block.render(area, buf);

        // Render message
        let message = Paragraph::new(self.message).alignment(Alignment::Center);
        let message_area = Rect::new(inner.x, inner.y + 1, inner.width, 1);
        message.render(message_area, buf);

        // Render buttons
        let yes_style = if self.selected == DialogButton::Yes {
            self.selected_style
        } else {
            self.unselected_style
        };
        let no_style = if self.selected == DialogButton::No {
            self.selected_style
        } else {
            self.unselected_style
        };

        let yes_text = if self.selected == DialogButton::Yes {
            "[ Yes ]"
        } else {
            "  Yes  "
        };
        let no_text = if self.selected == DialogButton::No {
            "[ No ]"
        } else {
            "  No  "
        };

        let buttons = Line::from(vec![
            Span::styled(yes_text, yes_style),
            Span::raw("   "),
            Span::styled(no_text, no_style),
        ]);

        let buttons_paragraph = Paragraph::new(buttons).alignment(Alignment::Center);
        let buttons_area = Rect::new(
            inner.x,
            inner.y + inner.height.saturating_sub(2),
            inner.width,
            1,
        );
        buttons_paragraph.render(buttons_area, buf);

        // Render hint
        let hint = Paragraph::new("y/n • Enter • ←→ • Click")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        let hint_area = Rect::new(
            inner.x,
            inner.y + inner.height.saturating_sub(1),
            inner.width,
            1,
        );
        hint.render(hint_area, buf);
    }
}

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
    fn default_block(&self) -> Block<'a> {
        let style = if self.focused {
            self.focus_style
        } else {
            Style::default().fg(Color::DarkGray)
        };

        Block::default().borders(Borders::ALL).border_style(style)
    }
}

impl Widget for PaneWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Get screen state synchronously
        let screen = self.handle.screen().read().expect("screen lock poisoned");
        let vt_screen = screen.screen();

        // Determine the block to use
        let block = match self.block {
            Some(b) => b,
            None => self.default_block(),
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

/// Widget for rendering an empty bordered sub-panel.
pub struct SubPanelWidget<'a> {
    /// Optional title for the border.
    title: Option<&'a str>,
    /// Border style.
    border_style: Style,
}

impl<'a> SubPanelWidget<'a> {
    /// Create a new sub-panel widget.
    #[must_use]
    pub fn new() -> Self {
        Self {
            title: None,
            border_style: Style::default().fg(Color::DarkGray),
        }
    }

    /// Set the title.
    #[must_use]
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Set the border style.
    #[must_use]
    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }
}

impl Default for SubPanelWidget<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for SubPanelWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.border_style);

        if let Some(title) = self.title {
            block = block.title(title);
        }

        block.render(area, buf);
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
    /// Sub-panel areas for rendering.
    sub_panel_areas: &'a [Rect],
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
            sub_panel_areas: &[],
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

    /// Set sub-panel areas to render.
    #[must_use]
    pub fn sub_panels(mut self, areas: &'a [Rect]) -> Self {
        self.sub_panel_areas = areas;
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
                    .border_style(border_style);

                let widget = PaneWidget::new(handle)
                    .focused(is_focused)
                    .block(block)
                    .focus_style(self.focus_style);

                widget.render(*pane_area, buf);
            }
        }

        // Render sub-panels
        for sub_area in self.sub_panel_areas {
            let widget = SubPanelWidget::new().border_style(self.unfocus_style);
            widget.render(*sub_area, buf);
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
