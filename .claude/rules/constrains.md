---
paths:
  - "**/*.rs"
---

# Technical Constraints

- The language used for the project is Rust, and every commit must be formatted using `cargo fmt`.
- Use conventional commits for commit messages.
- Use TDD (Test-Driven Development) for writing tests before implementing functionality.
- Use Domain Driven Design (DDD) for organizing code and defining boundaries.
- Keep the codebase clean, maintainable, and easy to understand.
- Follow SOLID principles in every code change.
- Follow good practices and conventions that are designed for Rust.
- When making comments, use clear and concise language that is easy to understand. Do not use jargon or technical terms that are not familiar to the reader.
- Keep it updated, search the latest techonologies and libraries before implementing new functionality. THIS IS MANDATORY.
- Keep it simple, do not over engineer or add unnecessary complexity.

# Networking & Security Constraints

- Transport is UDP only. Never introduce TCP as a fallback.
- All UDP traffic must be wrapped in DTLS 1.3. Plaintext packets must be dropped immediately.
- Both peers must authenticate with certificates (mTLS). Anonymous connections are forbidden.
- Implement TOFU: store the peer's certificate fingerprint on first connect and reject changes on subsequent connections.
- Enable DTLS replay protection. Packets outside the replay window must be silently dropped.
- Maintain a per-machine allowlist of trusted peer addresses and certificate fingerprints. Reject unlisted peers before any input processing.
- Apply input event rate limiting per session to prevent flooding.
- The daemon must drop to least-privilege after startup. Use OS capabilities or sandbox APIs, never run as root unless strictly required by the input subsystem.
- TLS keys, session secrets, and private keys must never be written to logs, stdout, or debug output.
- Clipboard sharing must be opt-in and disabled by default.

# Quality Attributes

- Security: The system must be secure, with mutual authentication, TOFU, anti-replay protection, and least privilege.
- Reliability: The system must be reliable, with no data loss or corruption, and fast response times.
- Scalability: The system must be scalable, with the ability to handle increasing loads and data volumes.
- Maintainability: The system must be maintainable, with clear and concise code that is easy to understand and modify.

# Development Workflow

- Documentation must keep up with the code. Whenever a module (crate) is finished, update `docs/STATUS.md` in the same change: move that crate to "Implemented" with a short summary of what it provides, refresh the "What's next" roadmap, and bump the "Last updated" date.
