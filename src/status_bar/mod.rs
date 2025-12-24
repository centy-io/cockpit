//! Status bar widget for rendering plugin content at the top of the terminal.

mod segment;

pub use segment::StatusBarSegment;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

/// The height of the status bar (always 1 row).
pub const STATUS_BAR_HEIGHT: u16 = 1;

/// Configuration for the status bar.
#[derive(Clone, Debug)]
pub struct StatusBarConfig {
    /// Background style.
    pub style: Style,
    /// Separator between segments.
    pub separator: String,
}

impl Default for StatusBarConfig {
    fn default() -> Self {
        Self {
            style: Style::default().bg(Color::DarkGray).fg(Color::White),
            separator: " | ".to_string(),
        }
    }
}

/// Status bar widget for rendering at the top of the terminal.
pub struct StatusBarWidget<'a> {
    segments: &'a [&'a StatusBarSegment],
    config: StatusBarConfig,
}

impl<'a> StatusBarWidget<'a> {
    /// Create a new status bar widget.
    #[must_use]
    pub fn new(segments: &'a [&'a StatusBarSegment]) -> Self {
        Self {
            segments,
            config: StatusBarConfig::default(),
        }
    }

    /// Set the configuration.
    #[must_use]
    pub fn config(mut self, config: StatusBarConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the style.
    #[must_use]
    pub fn style(mut self, style: Style) -> Self {
        self.config.style = style;
        self
    }
}

impl Widget for StatusBarWidget<'_> {
    #[allow(clippy::cast_possible_truncation)]
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Fill background
        for x in area.x..area.x + area.width {
            for y in area.y..area.y + area.height {
                buf[(x, y)].set_style(self.config.style);
            }
        }

        // Render segments
        let mut x = area.x + 1; // Small padding

        for (i, segment) in self.segments.iter().enumerate() {
            if segment.is_empty() {
                continue;
            }

            // Add separator between segments
            if i > 0 && x < area.x + area.width {
                let sep_width = self.config.separator.chars().count() as u16;
                if x + sep_width < area.x + area.width {
                    for (j, ch) in self.config.separator.chars().enumerate() {
                        buf[(x + j as u16, area.y)].set_char(ch);
                    }
                    x += sep_width;
                }
            }

            // Render icon if present
            if let Some(icon) = &segment.icon {
                for ch in icon.chars() {
                    if x >= area.x + area.width {
                        break;
                    }
                    buf[(x, area.y)].set_char(ch).set_style(segment.style);
                    x += 1;
                }
                // Space after icon
                if x < area.x + area.width {
                    x += 1;
                }
            }

            // Render content
            for ch in segment.content.chars() {
                if x >= area.x + area.width {
                    break;
                }
                buf[(x, area.y)].set_char(ch).set_style(segment.style);
                x += 1;
            }
        }
    }
}
