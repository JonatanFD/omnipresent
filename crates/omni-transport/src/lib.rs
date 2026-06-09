//! Transport: moves encoded messages between machines — the pipe.
//!
//! Owns the UDP socket and the DTLS 1.3 channel over it, frames and
//! (de)serializes Protocol messages to and from datagrams, enforces the
//! anti-replay window configured by Security, and drops anything that fails the
//! DTLS layer. UDP only — never a TCP fallback.
