//! Attribution proofs — the never-destroy core: what an existing entry
//! is, proven by symlink resolution, tree digest, or the sentinel hash —
//! never guessed.

use std::path::Path;

use super::{SENTINEL, SetupError, tree_digest};

/// What an existing install-target entry is, proven — never guessed.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Attribution {
    Absent,
    /// A symlink (canonical phase: the user manages it).
    SymlinkEntry,
    /// A plain file where a skill dir belongs.
    FileEntry,
    /// A tree whose digest equals the payload's — current, ours.
    Current,
    /// A tree the sentinel proves we wrote, now differing from the
    /// payload — ours, stale.
    OursStale,
    /// Anything else: not attributable to setup.
    Foreign,
}

pub(super) fn attribute(dest: &Path, payload: &str) -> Result<Attribution, SetupError> {
    if dest.is_symlink() {
        return Ok(Attribution::SymlinkEntry);
    }
    if !dest.exists() {
        return Ok(Attribution::Absent);
    }
    if !dest.is_dir() {
        return Ok(Attribution::FileEntry);
    }
    let digest = tree_digest(dest)?;
    if digest == payload {
        return Ok(Attribution::Current);
    }
    Ok(if sentinel_proves(dest, &digest) {
        Attribution::OursStale
    } else {
        Attribution::Foreign
    })
}

/// The sentinel's recorded sha256 matches the tree as it stands now.
pub(super) fn sentinel_proves(dest: &Path, digest: &str) -> bool {
    std::fs::read_to_string(dest.join(SENTINEL))
        .is_ok_and(|text| text.lines().nth(1) == Some(digest))
}

/// Does `path` resolve to somewhere under `canonical`?
pub(super) fn points_into(path: &Path, canonical: &Path) -> bool {
    match (path.canonicalize(), canonical.canonicalize()) {
        (Ok(resolved), Ok(base)) => resolved.starts_with(base),
        _ => false,
    }
}
