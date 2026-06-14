//! Topology: models the virtual desktop formed by all connected machines.
//!
//! Holds each machine's screen geometry and relative position, tracks the
//! cursor's virtual position, and decides when and where the cursor crosses
//! between machines — supplying the entry coordinates that make movement seamless.
//!
//! The arrangement is described with edges and neighbors rather than a global
//! coordinate plane: each machine knows which peer sits past each of its edges,
//! and a crossing lands on the neighbor's opposite edge with the position along
//! the shared edge mapped proportionally.

pub mod geometry;
pub mod layout;
pub mod store;

pub use geometry::{Edge, Point, Screen};
pub use layout::{Advance, Crossing, CursorState, LayoutError, Machine, VirtualLayout};
pub use store::{InMemoryLayoutStore, LayoutStore, NoLayoutStored};
