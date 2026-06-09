//! Security: the trust authority. Owns policy, not the socket.
//!
//! Verifies mTLS identities, pins peer fingerprints on first use (TOFU) and
//! rejects later changes, enforces the per-machine allowlist, and configures the
//! DTLS anti-replay window. Security informs Transport which peers and certs are
//! acceptable; Transport calls in to authorize a handshake.
