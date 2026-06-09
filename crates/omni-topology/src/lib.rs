//! Topology: models the virtual desktop formed by all connected machines.
//!
//! Holds each machine's screen geometry and relative position, tracks the
//! cursor's virtual position, and decides when and where the cursor crosses
//! between machines — supplying the entry coordinates that make movement seamless.
