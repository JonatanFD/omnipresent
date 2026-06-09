//! Shared kernel: the common vocabulary every other module speaks.
//!
//! Holds the value objects and wire messages exchanged between machines —
//! input events, control messages, shared identifiers, and the (de)serialization
//! format. This crate is the leaf of the dependency graph: it depends on nothing
//! internal, and everything else depends on it.
