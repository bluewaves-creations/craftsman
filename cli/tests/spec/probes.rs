//! Capability probes for the `@requires-*` scenario tags: each runs once
//! and is cached for the whole harness run. A failed probe excludes the
//! tagged scenarios — they stay visible as unknown in `spec status`.

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// Swift toolchain ≥ 6.2 (SE-0451 raw identifiers in generated tests).
pub fn swift() -> bool {
    static PROBE: OnceLock<bool> = OnceLock::new();
    *PROBE.get_or_init(|| swift_version().is_some_and(|v| v >= (6, 2)))
}

fn swift_version() -> Option<(u32, u32)> {
    let out = Command::new("swift").arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let rest = text.split("Swift version ").nth(1)?;
    let mut parts = rest.split(|c: char| !c.is_ascii_digit());
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// xcodebuild runnable AND the selected toolchain ≥ 6.2 — the generated
/// UI tests use SE-0451 raw identifiers, so a runnable xcodebuild with an
/// older Swift (CI's Xcode 16.4 / Swift 6.1) still cannot build them
/// (observed on CI run 29663007111: probe passed, build failed).
pub fn xcode() -> bool {
    static PROBE: OnceLock<bool> = OnceLock::new();
    // xcodebuild spells it `-version` (single dash).
    *PROBE.get_or_init(|| {
        swift()
            && Command::new("xcodebuild")
                .arg("-version")
                .output()
                .is_ok_and(|o| o.status.success())
    })
}

/// Playwright chromium plus the committed visual baseline for this
/// platform (mirrors `tests/runtime_gates.rs::preconditions`).
pub fn chromium() -> bool {
    static PROBE: OnceLock<bool> = OnceLock::new();
    *PROBE.get_or_init(|| {
        let baseline = static_site_fixture().join(format!(
            "tests/__screenshots__/home-{}.png",
            if cfg!(target_os = "macos") {
                "darwin"
            } else if cfg!(target_os = "windows") {
                "win32"
            } else {
                "linux"
            }
        ));
        chromium_binary().is_some() && baseline.is_file()
    })
}

pub fn static_site_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/static-site")
}

fn chromium_binary() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let cache = [
        home.join("Library/Caches/ms-playwright"),
        home.join(".cache/ms-playwright"),
    ]
    .into_iter()
    .find(|p| p.is_dir())?;
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&cache)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("chromium-"))
        })
        .collect();
    dirs.sort();
    let dir = dirs.pop()?;
    [
        dir.join("chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"),
        dir.join("chrome-mac/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"),
        dir.join("chrome-linux/chrome"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}
