//! Runtime: the composition root and long-running daemon.
//!
//! Wires concrete adapters into every module's ports and drives the
//! capture -> route -> send and receive -> inject pipelines. Exposes a local IPC
//! surface so the `omni` CLI can issue commands and receive notifications, and
//! applies least-privilege startup. The only place that depends on every module;
//! nothing depends on Runtime.
