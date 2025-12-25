//! Ratatui widgets for rendering panes.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::arrows::{is_left_arrow_position, ARROW_HEIGHT, ARROW_WIDTH, DOWN_ARROW, UP_ARROW};
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

/// Widget for rendering an empty bordered sub-pane.
pub struct SubPaneWidget<'a> {
    /// Optional title for the border.
    title: Option<&'a str>,
    /// Border style.
    border_style: Style,
}

impl<'a> SubPaneWidget<'a> {
    /// Create a new sub-pane widget.
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

impl Default for SubPaneWidget<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for SubPaneWidget<'_> {
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
    /// Sub-pane areas for rendering.
    sub_pane_areas: &'a [Rect],
    /// Empty pane areas (pane_number, Rect) for slots without active PTYs.
    empty_pane_areas: &'a [(usize, Rect)],
    /// Whether to show pane labels/PIDs.
    show_numbers: bool,
    /// Process IDs mapped by pane label (e.g., "110" -> 12345).
    pane_pids: std::collections::HashMap<&'static str, u32>,
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
            sub_pane_areas: &[],
            empty_pane_areas: &[],
            show_numbers: false,
            pane_pids: std::collections::HashMap::new(),
        }
    }

    /// Infer which pane positions are expanded from sub_pane_areas.
    /// A position is expanded if its sub-panes have zero size.
    fn infer_expanded_positions(&self) -> [bool; 4] {
        let mut expanded = [false; 4];
        // Sub-pane indices: 0-1 for position 0, 2-3 for position 1, 4-5 for position 2, 6-7 for position 3
        for position in 0..4 {
            let first_idx = position * 2;
            if let Some(sub_area) = self.sub_pane_areas.get(first_idx) {
                expanded[position] = sub_area.width == 0 || sub_area.height == 0;
            }
        }
        expanded
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

    /// Set sub-pane areas to render.
    #[must_use]
    pub fn sub_panes(mut self, areas: &'a [Rect]) -> Self {
        self.sub_pane_areas = areas;
        self
    }

    /// Set empty pane areas to render (slots without active PTYs).
    #[must_use]
    pub fn empty_panes(mut self, areas: &'a [(usize, Rect)]) -> Self {
        self.empty_pane_areas = areas;
        self
    }

    /// Enable pane numbering in borders.
    #[must_use]
    pub fn show_numbers(mut self, show: bool) -> Self {
        self.show_numbers = show;
        self
    }

    /// Set process ID for a pane by its label (e.g., "110", "121", "212").
    /// Valid labels: 110, 120, 210, 220, 111, 112, 121, 122, 211, 212, 221, 222
    #[must_use]
    pub fn pane_pid(mut self, label: &'static str, pid: u32) -> Self {
        self.pane_pids.insert(label, pid);
        self
    }
}

impl Widget for CockpitWidget<'_> {
    fn render(self, _area: Rect, buf: &mut Buffer) {
        // Infer which positions are expanded from sub_pane_areas
        let expanded_positions = self.infer_expanded_positions();

        // Pane labels: positions 1-4 (panes) and 5-12 (sub-panes)
        const PANE_LABELS: [&str; 4] = ["110", "120", "210", "220"];
        const SUB_PANE_LABELS: [&str; 8] = ["111", "112", "121", "122", "211", "212", "221", "222"];

        // Create a lookup for pane handles
        let pane_map: std::collections::HashMap<_, _> =
            self.panes.iter().map(|(id, h)| (*id, *h)).collect();

        // Sort areas by x coordinate to get correct position order (left to right)
        let mut sorted_areas: Vec<_> = self.areas.iter().collect();
        sorted_areas.sort_by_key(|(_, rect)| rect.x);

        // Render each pane in its area
        for (idx, (pane_id, pane_area)) in sorted_areas.iter().enumerate() {
            if let Some(handle) = pane_map.get(pane_id) {
                let is_focused = self.focused == Some(*pane_id);
                let border_style = if is_focused {
                    self.focus_style
                } else {
                    self.unfocus_style
                };

                // First pane: ALL borders
                // Others: TOP + BOTTOM + RIGHT (no LEFT to avoid double border)
                let borders = if idx == 0 {
                    Borders::ALL
                } else {
                    Borders::TOP | Borders::BOTTOM | Borders::RIGHT
                };

                let block = Block::default().borders(borders).border_style(border_style);

                let widget = PaneWidget::new(handle)
                    .focused(is_focused)
                    .block(block)
                    .focus_style(self.focus_style);

                widget.render(*pane_area, buf);

                // Render up arrow on expanded panes
                if idx < 4 && expanded_positions[idx] {
                    let arrow_style = Style::default().fg(Color::White);
                    let base_y = pane_area.y + pane_area.height - 1 - ARROW_HEIGHT;
                    let base_x = if is_left_arrow_position(idx) {
                        pane_area.x + 1
                    } else {
                        pane_area.x + pane_area.width - 1 - ARROW_WIDTH
                    };

                    for (row, line) in UP_ARROW.iter().enumerate() {
                        let y = base_y + row as u16;
                        if y >= buf.area.y + buf.area.height {
                            continue;
                        }
                        for (col, &ch) in line.iter().enumerate() {
                            let x = base_x + col as u16;
                            if x < buf.area.x || x >= buf.area.x + buf.area.width {
                                continue;
                            }
                            if ch != ' ' {
                                let cell = &mut buf[(x, y)];
                                cell.set_char(ch);
                                cell.set_style(arrow_style);
                            }
                        }
                    }
                }

                // Show PID or label as centered content
                if self.show_numbers {
                    let inner = Block::default().borders(borders).inner(*pane_area);
                    let label = PANE_LABELS.get(idx).unwrap_or(&"");
                    let display_text = match self.pane_pids.get(label) {
                        Some(pid) => format!("{}", pid),
                        None => label.to_string(),
                    };
                    let paragraph = Paragraph::new(display_text)
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(Color::DarkGray));
                    let centered_area = Rect {
                        x: inner.x,
                        y: inner.y + inner.height / 2,
                        width: inner.width,
                        height: 1,
                    };
                    paragraph.render(centered_area, buf);
                }
            }
        }

        // Render empty pane areas
        for (pane_number, empty_area) in self.empty_pane_areas {
            // First pane: ALL borders
            // Others: TOP + BOTTOM + RIGHT (no LEFT to avoid double border)
            let borders = if *pane_number == 1 {
                Borders::ALL
            } else {
                Borders::TOP | Borders::BOTTOM | Borders::RIGHT
            };

            let block = Block::default()
                .borders(borders)
                .border_style(self.unfocus_style);
            let inner = block.inner(*empty_area);
            block.render(*empty_area, buf);

            // Show PID or label as centered content
            if self.show_numbers {
                let idx = pane_number - 1;
                let label = PANE_LABELS.get(idx).unwrap_or(&"");
                let display_text = match self.pane_pids.get(label) {
                    Some(pid) => format!("{}", pid),
                    None => label.to_string(),
                };
                let paragraph = Paragraph::new(display_text)
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::DarkGray));
                let centered_area = Rect {
                    x: inner.x,
                    y: inner.y + inner.height / 2,
                    width: inner.width,
                    height: 1,
                };
                paragraph.render(centered_area, buf);
            }
        }

        // Render sub-panes
        for (idx, sub_area) in self.sub_pane_areas.iter().enumerate() {
            // Skip empty sub-panes (expanded positions have empty rects)
            if sub_area.width == 0 || sub_area.height == 0 {
                continue;
            }

            // First visible sub-pane gets LEFT border, others don't
            // Find if this is the first non-empty sub-pane
            let is_first_visible = self
                .sub_pane_areas
                .iter()
                .take(idx)
                .all(|r| r.width == 0 || r.height == 0);

            let borders = if is_first_visible {
                Borders::ALL
            } else {
                Borders::TOP | Borders::BOTTOM | Borders::RIGHT
            };

            let block = Block::default()
                .borders(borders)
                .border_style(self.unfocus_style);
            let inner = block.inner(*sub_area);
            block.render(*sub_area, buf);

            // Show PID or label as centered content
            if self.show_numbers {
                let label = SUB_PANE_LABELS.get(idx).unwrap_or(&"");
                let display_text = match self.pane_pids.get(label) {
                    Some(pid) => format!("{}", pid),
                    None => label.to_string(),
                };
                let paragraph = Paragraph::new(display_text)
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::DarkGray));
                let centered_area = Rect {
                    x: inner.x,
                    y: inner.y + inner.height / 2,
                    width: inner.width,
                    height: 1,
                };
                paragraph.render(centered_area, buf);
            }

            // Render down arrows for overlay navigation
            // 111 (idx 0): bottom-left, 122 (idx 3): bottom-right
            // 211 (idx 4): bottom-left, 222 (idx 7): bottom-right
            let arrow_style = Style::default().fg(Color::White);
            let base_y = sub_area.y + sub_area.height - 1 - ARROW_HEIGHT;

            let base_x = match idx {
                0 | 4 => Some(sub_area.x + 1), // Bottom-left
                3 | 7 => Some(sub_area.x + sub_area.width - 1 - ARROW_WIDTH), // Bottom-right
                _ => None,
            };

            if let Some(base_x) = base_x {
                for (row, line) in DOWN_ARROW.iter().enumerate() {
                    let y = base_y + row as u16;
                    if y >= buf.area.y + buf.area.height {
                        continue;
                    }
                    for (col, &ch) in line.iter().enumerate() {
                        let x = base_x + col as u16;
                        if x < buf.area.x || x >= buf.area.x + buf.area.width {
                            continue;
                        }
                        if ch != ' ' {
                            let cell = &mut buf[(x, y)];
                            cell.set_char(ch);
                            cell.set_style(arrow_style);
                        }
                    }
                }
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
