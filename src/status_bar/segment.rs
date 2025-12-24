//! Status bar segment - a unit of content from a plugin.

use ratatui::style::Style;

/// A segment of text for the status bar.
#[derive(Clone, Debug, Default)]
pub struct StatusBarSegment {
    /// The text content.
    pub content: String,
    /// Style for the segment.
    pub style: Style,
    /// Optional icon/prefix (symbol).
    pub icon: Option<String>,
    /// Minimum width (for alignment).
    pub min_width: Option<u16>,
}

impl StatusBarSegment {
    /// Create a new segment with text only.
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            ..Default::default()
        }
    }

    /// Set the style.
    #[must_use]
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Set an icon/prefix.
    #[must_use]
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Set minimum width.
    #[must_use]
    pub fn min_width(mut self, width: u16) -> Self {
        self.min_width = Some(width);
        self
    }

    /// Check if segment is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty() && self.icon.is_none()
    }

    /// Get display width (approximate).
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn width(&self) -> u16 {
        let icon_width = self.icon.as_ref().map_or(0, |i| i.chars().count() + 1);
        let content_width = self.content.chars().count();
        let total = (icon_width + content_width) as u16;
        self.min_width.map_or(total, |min| min.max(total))
    }
}
