//! Self-update against the release channel (Batch 10).
//!
//! Wraps axoupdater's receipt-driven flow: the install receipt written by
//! the cargo-dist installer (`~/.config/craftsman/craftsman-receipt.json`,
//! or `AXOUPDATER_CONFIG_PATH`) names the release source and installed
//! version. No receipt means this binary did not come from a release
//! install, and self-update honestly declines rather than guessing.
//!
//! This module is the one sanctioned network user in the bootstrap family;
//! nothing here runs in a verdict path.

use axoupdater::{AxoUpdater, AxoupdateError, ReleaseSourceType, UpdateRequest, Version};
use serde::Serialize;
use thiserror::Error;

/// Outcome of a self-update attempt. Every variant is an exit-0 report;
/// failures are [`UpdateError`].
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum SelfUpdate {
    /// No install receipt: not installed from a release. The caller prints
    /// the reinstall guidance (install.sh / cargo install).
    NoReceipt,
    /// A receipt exists but describes an install at a different location
    /// than the running binary (e.g. a debug build while a release install
    /// is present). Updating would touch the other install — declined.
    ForeignBinary { receipt_prefix: String },
    /// The release channel answered: this version is the latest.
    UpToDate { version: String },
    /// The new version was downloaded and installed over the receipt's
    /// install prefix. The running process is still the old binary.
    Updated {
        old: String,
        new: String,
        prefix: String,
    },
}

#[derive(Debug, Error)]
pub enum UpdateError {
    /// The receipt names a release channel that could not be queried —
    /// network down, host unreachable, release missing. Exit 1: the
    /// update verdict is "failed", never silently green.
    #[error("release channel {channel} is unreachable: {source}")]
    ChannelUnreachable {
        channel: String,
        source: AxoupdateError,
    },
    /// The download/install step itself failed after a successful check.
    #[error("update from release channel {channel} failed to install: {source}")]
    InstallFailed {
        channel: String,
        source: AxoupdateError,
    },
    /// Receipt lookup could not even determine a home directory.
    #[error("cannot look for an install receipt: {0}")]
    Environment(AxoupdateError),
    /// The running binary's version string is not semver — a build defect.
    #[error("running version {0:?} is not a semver version")]
    BadVersion(String),
}

/// Human-readable channel name from the loaded receipt,
/// e.g. `github:bluewaves-creations/craftsman`.
fn channel_name(updater: &AxoUpdater) -> String {
    updater.source.as_ref().map_or_else(
        || "unknown".to_owned(),
        |s| {
            let kind = match s.release_type {
                ReleaseSourceType::GitHub => "github",
                ReleaseSourceType::Axo => "axo",
            };
            format!("{kind}:{}/{}", s.owner, s.name)
        },
    )
}

/// Attempt a self-update of the running binary (version `current`,
/// pure semver) to the latest release named by the install receipt.
///
/// # Errors
///
/// [`UpdateError::ChannelUnreachable`] / [`UpdateError::InstallFailed`]
/// when the receipt's release channel cannot be queried or the install
/// step fails (exit 1 at the command layer);
/// [`UpdateError::Environment`] / [`UpdateError::BadVersion`] for a
/// missing home directory or a non-semver running version (exit 3).
#[allow(
    clippy::result_large_err,
    reason = "axoupdater's error variants embed reqwest errors; the type is \
              constructed once per process on a cold path"
)]
pub fn self_update(current: &str) -> Result<SelfUpdate, UpdateError> {
    let mut updater = AxoUpdater::new_for("craftsman");
    match updater.load_receipt() {
        Ok(_) => {}
        Err(
            AxoupdateError::NoReceipt { .. }
            | AxoupdateError::ReceiptLoadFailed { .. }
            | AxoupdateError::ConfigFetchFailed { .. },
        ) => return Ok(SelfUpdate::NoReceipt),
        Err(e) => return Err(UpdateError::Environment(e)),
    }
    let channel = channel_name(&updater);
    // Private release channels: honor the same token variable the
    // cargo-dist shell installer reads, so one export serves install,
    // update check, and the spawned installer alike.
    if let Ok(token) = std::env::var("CRAFTSMAN_GITHUB_TOKEN")
        && !token.is_empty()
    {
        updater.set_github_token(&token);
    }

    if !updater
        .check_receipt_is_for_this_executable()
        .unwrap_or(true)
    {
        let receipt_prefix = updater
            .install_prefix_root()
            .map_or_else(|_| "<unknown>".to_owned(), |p| p.to_string());
        return Ok(SelfUpdate::ForeignBinary { receipt_prefix });
    }

    let version =
        Version::parse(current).map_err(|_| UpdateError::BadVersion(current.to_owned()))?;
    updater
        .set_current_version(version)
        .map_err(UpdateError::Environment)?;
    updater.configure_version_specifier(UpdateRequest::Latest);
    // stdout stays reserved for `--json`; the installer may talk on stderr.
    updater.disable_installer_stdout();
    updater.enable_installer_stderr();

    check_and_run(updater, channel, current)
}

/// Query the channel and, when a newer release exists, install it.
#[allow(
    clippy::result_large_err,
    reason = "same axoupdater error payloads as self_update, same cold path"
)]
fn check_and_run(
    mut updater: AxoUpdater,
    channel: String,
    current: &str,
) -> Result<SelfUpdate, UpdateError> {
    let needed =
        updater
            .is_update_needed_sync()
            .map_err(|source| UpdateError::ChannelUnreachable {
                channel: channel.clone(),
                source,
            })?;
    if !needed {
        return Ok(SelfUpdate::UpToDate {
            version: current.to_owned(),
        });
    }

    match updater.run_sync() {
        Ok(Some(result)) => Ok(SelfUpdate::Updated {
            old: result
                .old_version
                .map_or_else(|| current.to_owned(), |v| v.to_string()),
            new: result.new_version.to_string(),
            prefix: result.install_prefix.to_string(),
        }),
        Ok(None) => Ok(SelfUpdate::UpToDate {
            version: current.to_owned(),
        }),
        Err(source) => Err(UpdateError::InstallFailed { channel, source }),
    }
}
