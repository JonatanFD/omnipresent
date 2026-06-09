//! The `LayoutStore` port: load and persist the configured arrangement of
//! machines. Real adapters (a config file, the OS config dir) live in the
//! Runtime; this module also ships an in-memory adapter for tests.

use crate::layout::VirtualLayout;

/// Persists and retrieves the virtual desktop layout. The domain depends only on
/// this trait, never on where the layout actually lives.
pub trait LayoutStore {
    /// What can go wrong loading or saving for this particular backend.
    type Error;

    /// Loads the saved layout.
    fn load(&self) -> Result<VirtualLayout, Self::Error>;

    /// Saves the layout, replacing any previously stored one.
    fn save(&mut self, layout: &VirtualLayout) -> Result<(), Self::Error>;
}

/// Returned by [`InMemoryLayoutStore::load`] when nothing has been saved yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoLayoutStored;

impl std::fmt::Display for NoLayoutStored {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("no layout has been stored")
    }
}

impl std::error::Error for NoLayoutStored {}

/// A `LayoutStore` that keeps the layout in memory. Useful for tests and for
/// running without persistence.
#[derive(Debug, Default)]
pub struct InMemoryLayoutStore {
    layout: Option<VirtualLayout>,
}

impl LayoutStore for InMemoryLayoutStore {
    type Error = NoLayoutStored;

    fn load(&self) -> Result<VirtualLayout, Self::Error> {
        self.layout.clone().ok_or(NoLayoutStored)
    }

    fn save(&mut self, layout: &VirtualLayout) -> Result<(), Self::Error> {
        self.layout = Some(layout.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Edge, Screen};
    use crate::layout::{Machine, VirtualLayout};
    use omni_protocol::MachineId;

    fn sample_layout() -> VirtualLayout {
        let mut layout = VirtualLayout::new();
        let a = MachineId::new(1);
        let b = MachineId::new(2);
        layout
            .add_machine(Machine::new(a, Screen::new(100, 100)))
            .unwrap();
        layout
            .add_machine(Machine::new(b, Screen::new(100, 100)))
            .unwrap();
        layout.link(a, Edge::Right, b).unwrap();
        layout
    }

    #[test]
    fn loading_before_any_save_reports_no_layout() {
        let store = InMemoryLayoutStore::default();
        assert_eq!(store.load(), Err(NoLayoutStored));
    }

    #[test]
    fn saved_layout_is_returned_by_load() {
        let mut store = InMemoryLayoutStore::default();
        let layout = sample_layout();

        store.save(&layout).unwrap();

        assert_eq!(store.load().unwrap(), layout);
    }
}
