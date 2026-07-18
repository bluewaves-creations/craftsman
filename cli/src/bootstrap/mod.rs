//! Bootstrap commands — `init`, `adopt`, `setup`, `update` (Batch 8).
//!
//! The CLI side of the craftsman-init skill family: non-interactive,
//! flags in / files out. The interview, judgment, and destructive-scope
//! confirmation are skill-side; these modules scaffold, track phase state,
//! and install the bundled skills mechanically.

pub mod adopt;
pub mod init;
pub mod setup;
mod templates;
