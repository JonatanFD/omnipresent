//! Plain geometric value objects: screen sizes, points, and edges.

use serde::{Deserialize, Serialize};

/// The pixel dimensions of a machine's screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
    pub width: u32,
    pub height: u32,
}

impl Screen {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// A cursor position, local to one machine's screen. Always kept within that
/// screen's bounds: `0..width` by `0..height`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Point {
    pub x: u32,
    pub y: u32,
}

impl Point {
    pub const fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }
}

/// One of the four screen edges. Used both to describe how machines are arranged
/// (which neighbor sits past which edge) and to report where the cursor leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    /// The edge you enter from when crossing onto a neighbor. Crossing off the
    /// right edge of one screen puts you on the left edge of the next.
    pub const fn opposite(self) -> Edge {
        match self {
            Edge::Left => Edge::Right,
            Edge::Right => Edge::Left,
            Edge::Top => Edge::Bottom,
            Edge::Bottom => Edge::Top,
        }
    }

    /// Whether crossing this edge is horizontal movement (left/right). The
    /// perpendicular axis — the one mapped onto the neighbor — is then vertical.
    pub const fn is_horizontal(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opposite_edges_pair_up() {
        assert_eq!(Edge::Left.opposite(), Edge::Right);
        assert_eq!(Edge::Right.opposite(), Edge::Left);
        assert_eq!(Edge::Top.opposite(), Edge::Bottom);
        assert_eq!(Edge::Bottom.opposite(), Edge::Top);
    }

    #[test]
    fn left_and_right_are_horizontal() {
        assert!(Edge::Left.is_horizontal());
        assert!(Edge::Right.is_horizontal());
        assert!(!Edge::Top.is_horizontal());
        assert!(!Edge::Bottom.is_horizontal());
    }
}
