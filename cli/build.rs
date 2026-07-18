//! Build metadata: bake the git sha into `--version` so a deployed binary
//! is traceable to its commit (team-local distribution, Batch 8).

use std::process::Command;

fn main() {
    let sha = Command::new("git")
        .args(["rev-parse", "--short=9", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map_or_else(
            || "unknown".to_owned(),
            |o| String::from_utf8_lossy(&o.stdout).trim().to_owned(),
        );
    println!("cargo:rustc-env=CRAFTSMAN_GIT_SHA={sha}");
    // Keep the sha honest across commits without watching the whole index.
    println!("cargo:rerun-if-changed=../.git/HEAD");
}
