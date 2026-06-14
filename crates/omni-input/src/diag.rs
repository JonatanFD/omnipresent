//! Self-diagnosis: did the OS give us what capture and injection need?
//!
//! Each platform module exposes `diagnose() -> Vec<Check>` describing its
//! permission requirements and whether they are currently met. The `omni
//! doctor` command prints these so a half-working setup (e.g. Accessibility
//! revoked by a rebuild on macOS, or a missing `input` group on Linux) is
//! visible at a glance instead of silently degrading to target-only.

/// One environment requirement and whether it is currently satisfied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// What is being checked, e.g. "accessibility permission".
    pub name: &'static str,
    /// Whether the requirement is met right now.
    pub ok: bool,
    /// What was found — and, when not ok, how to fix it.
    pub detail: String,
}

impl Check {
    pub fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            ok: true,
            detail: detail.into(),
        }
    }

    pub fn failed(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            ok: false,
            detail: detail.into(),
        }
    }
}
