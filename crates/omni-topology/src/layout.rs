//! The virtual desktop: which machines exist, how they are arranged, and where
//! the cursor goes when it is moved.

use crate::geometry::{Edge, Point, Screen};
use omni_protocol::MachineId;
use omni_protocol::input::MouseDelta;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A machine and the screen it contributes to the virtual desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Machine {
    pub id: MachineId,
    pub screen: Screen,
}

impl Machine {
    pub const fn new(id: MachineId, screen: Screen) -> Self {
        Self { id, screen }
    }
}

/// Something the caller asked the layout to do that does not make sense.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    /// Referenced a machine that was never added.
    UnknownMachine(MachineId),
    /// Tried to add a machine id that already exists.
    DuplicateMachine(MachineId),
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutError::UnknownMachine(id) => write!(f, "unknown machine {}", id.value()),
            LayoutError::DuplicateMachine(id) => write!(f, "duplicate machine {}", id.value()),
        }
    }
}

impl std::error::Error for LayoutError {}

/// Where the cursor currently is: which machine, and where on its screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorState {
    pub machine: MachineId,
    pub position: Point,
}

impl CursorState {
    pub const fn new(machine: MachineId, position: Point) -> Self {
        Self { machine, position }
    }
}

/// The cursor left the current machine and entered a neighbor at `entry`, in the
/// neighbor's own screen coordinates. This is what makes movement seamless and
/// what tells Session to flip the active Target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crossing {
    pub peer: MachineId,
    pub entry: Point,
}

/// The result of moving the cursor by one delta. `cursor` is always the new
/// position; `crossing` is set only when it moved onto a neighbor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Advance {
    pub cursor: CursorState,
    pub crossing: Option<Crossing>,
}

/// The configured arrangement of machines: each machine's screen plus which
/// neighbor sits past each edge. This is the value persisted by a `LayoutStore`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualLayout {
    screens: HashMap<MachineId, Screen>,
    /// Maps a machine's edge to the neighbor reached across it. Kept symmetric:
    /// linking A's right to B also links B's left to A.
    links: HashMap<(MachineId, Edge), MachineId>,
}

impl VirtualLayout {
    /// An empty layout with no machines.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a machine. Fails if one with the same id is already present.
    pub fn add_machine(&mut self, machine: Machine) -> Result<(), LayoutError> {
        if self.screens.contains_key(&machine.id) {
            return Err(LayoutError::DuplicateMachine(machine.id));
        }
        self.screens.insert(machine.id, machine.screen);
        Ok(())
    }

    /// The screen of a known machine, if present.
    pub fn screen(&self, machine: MachineId) -> Option<Screen> {
        self.screens.get(&machine).copied()
    }

    /// Links machine `a`'s `edge` to neighbor `b`, and symmetrically `b`'s
    /// opposite edge back to `a`. Both machines must already exist.
    pub fn link(&mut self, a: MachineId, edge: Edge, b: MachineId) -> Result<(), LayoutError> {
        if !self.screens.contains_key(&a) {
            return Err(LayoutError::UnknownMachine(a));
        }
        if !self.screens.contains_key(&b) {
            return Err(LayoutError::UnknownMachine(b));
        }
        self.links.insert((a, edge), b);
        self.links.insert((b, edge.opposite()), a);
        Ok(())
    }

    /// The neighbor reached by crossing `machine`'s `edge`, if any.
    pub fn neighbor(&self, machine: MachineId, edge: Edge) -> Option<MachineId> {
        self.links.get(&(machine, edge)).copied()
    }

    /// Moves the cursor by `delta`. If it stays on the current screen it is
    /// returned at the new spot; if it runs past an edge with a neighbor it
    /// crosses; if it runs past an edge with no neighbor it is clamped to the
    /// boundary and stays put.
    pub fn advance(&self, cursor: CursorState, delta: MouseDelta) -> Result<Advance, LayoutError> {
        let screen = self
            .screen(cursor.machine)
            .ok_or(LayoutError::UnknownMachine(cursor.machine))?;

        let cand_x = cursor.position.x as i64 + delta.dx as i64;
        let cand_y = cursor.position.y as i64 + delta.dy as i64;
        let w = screen.width as i64;
        let h = screen.height as i64;

        // Which edges did the move run past, and by how much?
        let horizontal = edge_overshoot(cand_x, w, Edge::Left, Edge::Right);
        let vertical = edge_overshoot(cand_y, h, Edge::Top, Edge::Bottom);

        // Of the edges that were crossed and actually have a neighbor, take the
        // one we overshot most — that is the direction the cursor was heading.
        let chosen = [horizontal, vertical]
            .into_iter()
            .flatten()
            .filter_map(|(edge, over)| self.neighbor(cursor.machine, edge).map(|p| (edge, over, p)))
            .max_by_key(|&(_, over, _)| over);

        if let Some((edge, _, peer)) = chosen {
            let peer_screen = self.screen(peer).ok_or(LayoutError::UnknownMachine(peer))?;
            let entry = entry_point(edge, peer_screen, screen, cand_x, cand_y);
            let cursor = CursorState::new(peer, entry);
            return Ok(Advance {
                cursor,
                crossing: Some(Crossing { peer, entry }),
            });
        }

        // No crossing: keep the cursor on this screen, clamped into bounds.
        let position = Point::new(cand_x.clamp(0, w - 1) as u32, cand_y.clamp(0, h - 1) as u32);
        Ok(Advance {
            cursor: CursorState::new(cursor.machine, position),
            crossing: None,
        })
    }
}

/// Reports which edge a candidate coordinate ran past and by how much, or `None`
/// if it stayed within `0..length`.
fn edge_overshoot(coord: i64, length: i64, low: Edge, high: Edge) -> Option<(Edge, i64)> {
    if coord < 0 {
        Some((low, -coord))
    } else if coord >= length {
        Some((high, coord - (length - 1)))
    } else {
        None
    }
}

/// Computes where the cursor lands on the neighbor's screen after crossing
/// `edge`. The coordinate along the shared edge is mapped proportionally so the
/// cursor keeps its relative position even across differently sized screens.
fn entry_point(edge: Edge, peer: Screen, from: Screen, cand_x: i64, cand_y: i64) -> Point {
    if edge.is_horizontal() {
        // Crossing left/right: x snaps to the entry side, y maps along height.
        let y = project(cand_y, from.height, peer.height);
        let x = match edge {
            Edge::Right => 0,
            Edge::Left => peer.width.saturating_sub(1),
            _ => unreachable!("horizontal edge is left or right"),
        };
        Point::new(x, y)
    } else {
        // Crossing top/bottom: y snaps to the entry side, x maps along width.
        let x = project(cand_x, from.width, peer.width);
        let y = match edge {
            Edge::Bottom => 0,
            Edge::Top => peer.height.saturating_sub(1),
            _ => unreachable!("vertical edge is top or bottom"),
        };
        Point::new(x, y)
    }
}

/// Maps a coordinate from a span of length `src` onto a span of length `dst`,
/// keeping the same relative position (0 maps to 0, `src-1` maps to `dst-1`).
fn project(coord: i64, src: u32, dst: u32) -> u32 {
    if src <= 1 || dst == 0 {
        return 0;
    }
    let max_src = (src - 1) as i64;
    let clamped = coord.clamp(0, max_src) as u64;
    (clamped * (dst as u64 - 1) / (src as u64 - 1)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: MachineId = MachineId::new(1);
    const B: MachineId = MachineId::new(2);

    fn two_screens(a: Screen, b: Screen) -> VirtualLayout {
        let mut layout = VirtualLayout::new();
        layout.add_machine(Machine::new(A, a)).unwrap();
        layout.add_machine(Machine::new(B, b)).unwrap();
        layout
    }

    #[test]
    fn adding_the_same_machine_twice_fails() {
        let mut layout = VirtualLayout::new();
        layout
            .add_machine(Machine::new(A, Screen::new(100, 100)))
            .unwrap();

        assert_eq!(
            layout.add_machine(Machine::new(A, Screen::new(200, 200))),
            Err(LayoutError::DuplicateMachine(A)),
        );
    }

    #[test]
    fn linking_is_symmetric() {
        let mut layout = two_screens(Screen::new(100, 100), Screen::new(100, 100));
        layout.link(A, Edge::Right, B).unwrap();

        assert_eq!(layout.neighbor(A, Edge::Right), Some(B));
        assert_eq!(layout.neighbor(B, Edge::Left), Some(A));
        assert_eq!(layout.neighbor(A, Edge::Left), None);
    }

    #[test]
    fn linking_an_unknown_machine_fails() {
        let mut layout = VirtualLayout::new();
        layout
            .add_machine(Machine::new(A, Screen::new(100, 100)))
            .unwrap();

        assert_eq!(
            layout.link(A, Edge::Right, B),
            Err(LayoutError::UnknownMachine(B))
        );
    }

    #[test]
    fn moving_within_a_screen_does_not_cross() {
        let layout = two_screens(Screen::new(100, 100), Screen::new(100, 100));
        let cursor = CursorState::new(A, Point::new(10, 10));

        let advance = layout.advance(cursor, MouseDelta::new(5, -3)).unwrap();

        assert_eq!(advance.crossing, None);
        assert_eq!(advance.cursor, CursorState::new(A, Point::new(15, 7)));
    }

    #[test]
    fn hitting_an_edge_with_no_neighbor_clamps_and_stays() {
        let layout = two_screens(Screen::new(100, 100), Screen::new(100, 100));
        let cursor = CursorState::new(A, Point::new(98, 50));

        // Push far past the right edge; A has no right neighbor (not linked).
        let advance = layout.advance(cursor, MouseDelta::new(50, 0)).unwrap();

        assert_eq!(advance.crossing, None);
        assert_eq!(advance.cursor, CursorState::new(A, Point::new(99, 50)));
    }

    #[test]
    fn crossing_the_right_edge_enters_the_neighbor_from_the_left() {
        let mut layout = two_screens(Screen::new(100, 100), Screen::new(100, 100));
        layout.link(A, Edge::Right, B).unwrap();
        let cursor = CursorState::new(A, Point::new(99, 40));

        let advance = layout.advance(cursor, MouseDelta::new(5, 0)).unwrap();

        assert_eq!(advance.cursor.machine, B);
        assert_eq!(advance.cursor.position, Point::new(0, 40));
        assert_eq!(
            advance.crossing,
            Some(Crossing {
                peer: B,
                entry: Point::new(0, 40)
            })
        );
    }

    #[test]
    fn crossing_maps_position_proportionally_across_different_heights() {
        // A is 100 tall, B is 1000 tall. Halfway down A should be halfway down B.
        let mut layout = two_screens(Screen::new(100, 101), Screen::new(100, 1001));
        layout.link(A, Edge::Right, B).unwrap();
        let cursor = CursorState::new(A, Point::new(99, 50));

        let advance = layout.advance(cursor, MouseDelta::new(2, 0)).unwrap();

        assert_eq!(advance.cursor.machine, B);
        assert_eq!(advance.cursor.position, Point::new(0, 500));
    }

    #[test]
    fn crossing_the_bottom_edge_enters_the_neighbor_from_the_top() {
        let mut layout = two_screens(Screen::new(100, 100), Screen::new(100, 100));
        layout.link(A, Edge::Bottom, B).unwrap();
        let cursor = CursorState::new(A, Point::new(30, 99));

        let advance = layout.advance(cursor, MouseDelta::new(0, 5)).unwrap();

        assert_eq!(advance.cursor.machine, B);
        assert_eq!(advance.cursor.position, Point::new(30, 0));
        assert_eq!(advance.crossing.unwrap().peer, B);
    }

    #[test]
    fn advancing_an_unknown_cursor_machine_fails() {
        let layout = VirtualLayout::new();
        let cursor = CursorState::new(A, Point::new(0, 0));

        assert_eq!(
            layout.advance(cursor, MouseDelta::new(1, 0)),
            Err(LayoutError::UnknownMachine(A)),
        );
    }
}
