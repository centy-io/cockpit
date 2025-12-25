//! Arrow definitions for pane navigation.
//!
//! This module contains the visual representations and dimensions
//! of navigation arrows used for expanding/collapsing panes.

use ratatui::layout::Rect;

use crate::pane::PaneId;

/// Arrow dimensions (width x height in terminal cells).
pub const ARROW_WIDTH: u16 = 5;
pub const ARROW_HEIGHT: u16 = 3;

/// Down arrow ASCII art (5 wide x 3 tall).
/// Used on sub-panes to indicate "click to expand".
///
/// ```text
/// ╲   ╱
///  ╲ ╱
///   V
/// ```
pub const DOWN_ARROW: [[char; 5]; 3] = [
    ['╲', ' ', ' ', ' ', '╱'],
    [' ', '╲', ' ', '╱', ' '],
    [' ', ' ', 'V', ' ', ' '],
];

/// Up arrow ASCII art (5 wide x 3 tall).
/// Used on expanded panes to indicate "click to collapse".
///
/// ```text
///   ^
///  ╱ ╲
/// ╱   ╲
/// ```
pub const UP_ARROW: [[char; 5]; 3] = [
    [' ', ' ', '^', ' ', ' '],
    [' ', '╱', ' ', '╲', ' '],
    ['╱', ' ', ' ', ' ', '╲'],
];

/// Arrow positions for overlay navigation.
/// These correspond to clickable arrows in the sub-panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowPosition {
    /// Bottom-left arrow in sub-pane 111 (under pane 110)
    Pane111,
    /// Bottom-right arrow in sub-pane 122 (under pane 120)
    Pane122,
    /// Bottom-left arrow in sub-pane 211 (under pane 210)
    Pane211,
    /// Bottom-right arrow in sub-pane 222 (under pane 220)
    Pane222,
}

impl ArrowPosition {
    /// Get the pane position index (0-3) that this arrow controls.
    pub fn pane_position(self) -> usize {
        match self {
            Self::Pane111 => 0, // Controls pane 110 (position 0)
            Self::Pane122 => 1, // Controls pane 120 (position 1)
            Self::Pane211 => 2, // Controls pane 210 (position 2)
            Self::Pane222 => 3, // Controls pane 220 (position 3)
        }
    }
}

/// Check if a click at (x, y) hits any up arrow on expanded panes.
/// Returns the pane position (0-3) if an up arrow was clicked, None otherwise.
pub fn up_arrow_at_position(
    x: u16,
    y: u16,
    pane_areas: &[(PaneId, Rect)],
    expanded_positions: &[bool; 4],
) -> Option<usize> {
    for (idx, (_, pane_area)) in pane_areas.iter().enumerate() {
        if idx >= 4 || !expanded_positions[idx] {
            continue;
        }

        let base_y = pane_area.y + pane_area.height.saturating_sub(1 + ARROW_HEIGHT);

        // Position 0 and 2: bottom-left, Position 1 and 3: bottom-right
        let is_left = idx == 0 || idx == 2;
        let base_x = if is_left {
            pane_area.x + 1
        } else {
            pane_area.x + pane_area.width.saturating_sub(1 + ARROW_WIDTH)
        };

        // Check if click is within arrow bounds
        if x >= base_x
            && x < base_x + ARROW_WIDTH
            && y >= base_y
            && y < base_y + ARROW_HEIGHT
        {
            return Some(idx);
        }
    }

    None
}

/// Check if a click at (x, y) hits any of the navigation arrows (down arrows on sub-panes).
/// Returns the arrow position if clicked, None otherwise.
pub fn down_arrow_at_position(x: u16, y: u16, sub_pane_areas: &[Rect]) -> Option<ArrowPosition> {
    // Arrow indices: 111=0, 122=3, 211=4, 222=7
    let arrow_configs: [(usize, ArrowPosition, bool); 4] = [
        (0, ArrowPosition::Pane111, true),  // idx 0, left-aligned
        (3, ArrowPosition::Pane122, false), // idx 3, right-aligned
        (4, ArrowPosition::Pane211, true),  // idx 4, left-aligned
        (7, ArrowPosition::Pane222, false), // idx 7, right-aligned
    ];

    for (idx, position, is_left) in arrow_configs {
        if let Some(sub_area) = sub_pane_areas.get(idx) {
            // Skip empty sub-panes (expanded positions)
            if sub_area.width == 0 || sub_area.height == 0 {
                continue;
            }

            let base_y = sub_area.y + sub_area.height.saturating_sub(1 + ARROW_HEIGHT);
            let base_x = if is_left {
                sub_area.x + 1
            } else {
                sub_area.x + sub_area.width.saturating_sub(1 + ARROW_WIDTH)
            };

            // Check if click is within arrow bounds
            if x >= base_x
                && x < base_x + ARROW_WIDTH
                && y >= base_y
                && y < base_y + ARROW_HEIGHT
            {
                return Some(position);
            }
        }
    }

    None
}

/// Returns whether a position should have its arrow on the left side.
/// Positions 0 and 2 have left arrows, positions 1 and 3 have right arrows.
pub const fn is_left_arrow_position(position: usize) -> bool {
    position == 0 || position == 2
}
