//! Layout system for arranging panes.

use std::collections::HashMap;

use ratatui::layout::Rect;

use crate::pane::PaneId;

/// Split direction for layouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Split horizontally (top and bottom).
    Horizontal,
    /// Split vertically (left and right).
    Vertical,
}

/// Layout configuration for panes.
#[derive(Clone, Debug)]
pub enum Layout {
    /// Single pane filling the entire area.
    Single(PaneId),

    /// Split layout with two children.
    Split {
        /// Direction of the split.
        direction: Direction,
        /// Ratio for the first child (0.0 to 1.0).
        ratio: f32,
        /// First child layout.
        first: Box<Layout>,
        /// Second child layout.
        second: Box<Layout>,
    },
}

impl Layout {
    /// Create a single pane layout.
    #[must_use]
    pub fn single(pane_id: PaneId) -> Self {
        Self::Single(pane_id)
    }

    /// Create a horizontal split (top and bottom).
    #[must_use]
    pub fn hsplit(ratio: f32, first: Layout, second: Layout) -> Self {
        Self::Split {
            direction: Direction::Horizontal,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Create a vertical split (left and right).
    #[must_use]
    pub fn vsplit(ratio: f32, first: Layout, second: Layout) -> Self {
        Self::Split {
            direction: Direction::Vertical,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Create an equal horizontal split.
    #[must_use]
    pub fn hsplit_equal(first: Layout, second: Layout) -> Self {
        Self::hsplit(0.5, first, second)
    }

    /// Create an equal vertical split.
    #[must_use]
    pub fn vsplit_equal(first: Layout, second: Layout) -> Self {
        Self::vsplit(0.5, first, second)
    }

    /// Get all pane IDs in this layout.
    #[must_use]
    pub fn pane_ids(&self) -> Vec<PaneId> {
        let mut ids = Vec::new();
        self.collect_pane_ids(&mut ids);
        ids
    }

    fn collect_pane_ids(&self, ids: &mut Vec<PaneId>) {
        match self {
            Self::Single(id) => ids.push(*id),
            Self::Split { first, second, .. } => {
                first.collect_pane_ids(ids);
                second.collect_pane_ids(ids);
            }
        }
    }

    /// Check if a pane ID is in this layout.
    #[must_use]
    pub fn contains(&self, pane_id: PaneId) -> bool {
        match self {
            Self::Single(id) => *id == pane_id,
            Self::Split { first, second, .. } => first.contains(pane_id) || second.contains(pane_id),
        }
    }
}

/// Calculates areas for each pane in a layout.
pub struct LayoutCalculator;

impl LayoutCalculator {
    /// Calculate the Rect for each visible pane given the total area.
    #[must_use]
    pub fn calculate_areas(layout: &Layout, area: Rect) -> HashMap<PaneId, Rect> {
        let mut areas = HashMap::new();
        Self::calculate_recursive(layout, area, &mut areas);
        areas
    }

    fn calculate_recursive(layout: &Layout, area: Rect, areas: &mut HashMap<PaneId, Rect>) {
        match layout {
            Layout::Single(id) => {
                areas.insert(*id, area);
            }
            Layout::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (first_area, second_area) = Self::split_area(area, *direction, *ratio);
                Self::calculate_recursive(first, first_area, areas);
                Self::calculate_recursive(second, second_area, areas);
            }
        }
    }

    fn split_area(area: Rect, direction: Direction, ratio: f32) -> (Rect, Rect) {
        match direction {
            Direction::Horizontal => {
                // Top and bottom split
                let first_height = (f32::from(area.height) * ratio).round() as u16;
                let second_height = area.height.saturating_sub(first_height);

                let first_area = Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: first_height,
                };
                let second_area = Rect {
                    x: area.x,
                    y: area.y.saturating_add(first_height),
                    width: area.width,
                    height: second_height,
                };
                (first_area, second_area)
            }
            Direction::Vertical => {
                // Left and right split
                let first_width = (f32::from(area.width) * ratio).round() as u16;
                let second_width = area.width.saturating_sub(first_width);

                let first_area = Rect {
                    x: area.x,
                    y: area.y,
                    width: first_width,
                    height: area.height,
                };
                let second_area = Rect {
                    x: area.x.saturating_add(first_width),
                    y: area.y,
                    width: second_width,
                    height: area.height,
                };
                (first_area, second_area)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_layout() {
        let pane_id = PaneId(1);
        let layout = Layout::single(pane_id);
        let area = Rect::new(0, 0, 100, 50);

        let areas = LayoutCalculator::calculate_areas(&layout, area);

        assert_eq!(areas.len(), 1);
        assert_eq!(areas.get(&pane_id), Some(&area));
    }

    #[test]
    fn test_hsplit_layout() {
        let pane1 = PaneId(1);
        let pane2 = PaneId(2);
        let layout = Layout::hsplit_equal(Layout::single(pane1), Layout::single(pane2));
        let area = Rect::new(0, 0, 100, 50);

        let areas = LayoutCalculator::calculate_areas(&layout, area);

        assert_eq!(areas.len(), 2);
        assert_eq!(areas.get(&pane1), Some(&Rect::new(0, 0, 100, 25)));
        assert_eq!(areas.get(&pane2), Some(&Rect::new(0, 25, 100, 25)));
    }

    #[test]
    fn test_vsplit_layout() {
        let pane1 = PaneId(1);
        let pane2 = PaneId(2);
        let layout = Layout::vsplit_equal(Layout::single(pane1), Layout::single(pane2));
        let area = Rect::new(0, 0, 100, 50);

        let areas = LayoutCalculator::calculate_areas(&layout, area);

        assert_eq!(areas.len(), 2);
        assert_eq!(areas.get(&pane1), Some(&Rect::new(0, 0, 50, 50)));
        assert_eq!(areas.get(&pane2), Some(&Rect::new(50, 0, 50, 50)));
    }

    #[test]
    fn test_pane_ids() {
        let pane1 = PaneId(1);
        let pane2 = PaneId(2);
        let pane3 = PaneId(3);

        let layout = Layout::vsplit(
            0.6,
            Layout::single(pane1),
            Layout::hsplit_equal(Layout::single(pane2), Layout::single(pane3)),
        );

        let ids = layout.pane_ids();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&pane1));
        assert!(ids.contains(&pane2));
        assert!(ids.contains(&pane3));
    }

    #[test]
    fn test_contains() {
        let pane1 = PaneId(1);
        let pane2 = PaneId(2);
        let pane3 = PaneId(3);

        let layout = Layout::vsplit_equal(Layout::single(pane1), Layout::single(pane2));

        assert!(layout.contains(pane1));
        assert!(layout.contains(pane2));
        assert!(!layout.contains(pane3));
    }
}
