//! Runtime gates (visual / a11y / perf) live-proven against the committed
//! `fixtures/static-site/` fixture, through the real CLI path
//! (`craftsman <gate> --json`) — closing the Batch 6b "schema-doc-
//! constructed samples" gap. Each gate is driven to BOTH verdicts:
//!
//! - visual: green = index.html vs the committed baseline screenshot
//!   (`tests/__screenshots__/home-<platform>.png`, captured on this
//!   machine 2026-07-18 with `--update-snapshots`); red = the broken
//!   variant compared against the SAME baseline (shared snapshot dir).
//! - a11y: green = index.html through @axe-core/playwright; red = the
//!   seeded-issue variant (no lang, missing alt, low contrast).
//! - perf: green = lhci autorun with a generous budget; red = the absurd
//!   `total-byte-weight <= 1 byte` budget in lighthouserc-strict.json.
//!
//! Serving decision (recorded): the playwright specs load `file://` URLs —
//! no server at all; Lighthouse serves the dir itself via lhci's
//! `staticDistDir`. No `bunx serve`/`http.server` needed.
//!
//! Chrome decision (recorded): lhci's chrome-launcher needs a Chrome
//! binary; this machine has no system Chrome, so the test points
//! `CHROME_PATH` at the Playwright chromium already required for the
//! other gates (`bunx playwright install chromium` provisions it).
//!
//! Skip conditions — LOUD, never silent green: no Playwright chromium in
//! the ms-playwright cache, or no committed visual baseline for this
//! platform. Measured on this machine 2026-07-18: visual ~5s per phase,
//! a11y ~5s, perf ~25s (two lhci autoruns) — under the ignore threshold,
//! all run unignored.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/static-site")
}

/// The ms-playwright browser cache for this platform.
fn playwright_cache() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let home = PathBuf::from(home);
    let mac = home.join("Library/Caches/ms-playwright");
    let linux = home.join(".cache/ms-playwright");
    [mac, linux].into_iter().find(|p| p.is_dir())
}

/// The installed Playwright chromium executable, if any — the browser
/// probe AND lhci's `CHROME_PATH`.
fn chromium_binary() -> Option<PathBuf> {
    let cache = playwright_cache()?;
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
    let candidates = [
        dir.join("chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"),
        dir.join("chrome-mac/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"),
        dir.join("chrome-linux/chrome"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

const fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "win32"
    } else {
        "linux"
    }
}

/// Loud-skip probe: chromium + the committed baseline for this platform.
fn preconditions() -> Option<PathBuf> {
    let Some(chromium) = chromium_binary() else {
        eprintln!(
            "SKIPPED (loudly): no Playwright chromium in the ms-playwright \
             cache — run `bunx playwright install chromium` to enable the \
             live runtime-gate tests"
        );
        return None;
    };
    let baseline = fixture().join(format!("tests/__screenshots__/home-{}.png", platform()));
    if !baseline.is_file() {
        eprintln!(
            "SKIPPED (loudly): no committed visual baseline for this \
             platform at {} — capture one with `bunx playwright test \
             tests/visual.spec.ts --update-snapshots` and commit it",
            baseline.display()
        );
        return None;
    }
    Some(chromium)
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdirs");
    for entry in std::fs::read_dir(from).expect("read fixture dir") {
        let entry = entry.expect("dir entry");
        let src = entry.path();
        let dest = to.join(entry.file_name());
        if src.is_dir() {
            copy_tree(&src, &dest);
        } else {
            std::fs::copy(&src, &dest).unwrap_or_else(|e| panic!("copy {}: {e}", src.display()));
        }
    }
}

/// A disposable project: fixture files (sans `node_modules`) + bun install.
fn setup_project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    for name in ["site", "tests"] {
        copy_tree(&fixture().join(name), &dir.join(name));
    }
    for name in [
        "playwright.config.ts",
        "lighthouserc.json",
        "lighthouserc-strict.json",
        "package.json",
        "bun.lock",
    ] {
        std::fs::copy(fixture().join(name), dir.join(name))
            .unwrap_or_else(|e| panic!("copy {name}: {e}"));
    }
    let status = Command::new("bun")
        .args(["install", "--frozen-lockfile"])
        .current_dir(dir)
        .status()
        .expect("spawn bun install");
    assert!(status.success(), "bun install failed in {}", dir.display());
    tmp
}

fn write_config(dir: &Path, gate_section: &str) {
    std::fs::write(
        dir.join("craftsman.toml"),
        format!("[project]\nname = \"static-site\"\nstacks = [\"typescript\"]\n\n{gate_section}\n"),
    )
    .expect("write craftsman.toml");
}

/// Run `craftsman <gate> --json` with `CHROME_PATH` set (lhci needs it; the
/// playwright gates ignore it).
fn run_gate(dir: &Path, gate: &str, chromium: &Path) -> (Output, f64) {
    let started = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_craftsman"))
        .args([gate, "--json"])
        .env("CHROME_PATH", chromium)
        .current_dir(dir)
        .output()
        .expect("spawn craftsman");
    (output, started.elapsed().as_secs_f64())
}

fn parsed(output: &Output, context: &str) -> serde_json::Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "{context}: stdout not JSON ({e}):\n{stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_green(output: &Output, elapsed: f64, context: &str) {
    let doc = parsed(output, context);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{context}: expected green after {elapsed:.0}s:\n{doc:#}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(doc["passed"], true, "{context}: {doc:#}");
}

fn assert_red(output: &Output, elapsed: f64, context: &str) -> serde_json::Value {
    let doc = parsed(output, context);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{context}: expected the red verdict after {elapsed:.0}s:\n{doc:#}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(doc["passed"], false, "{context}: {doc:#}");
    assert!(
        doc["blocking"].as_u64().unwrap_or(0) > 0,
        "{context}: {doc:#}"
    );
    doc
}

#[test]
fn visual_gate_green_and_red_live() {
    let Some(chromium) = preconditions() else {
        return;
    };
    let tmp = setup_project();
    let dir = tmp.path();

    write_config(dir, "[visual]\ntest-glob = \"tests/visual.spec.ts\"");
    let (output, elapsed) = run_gate(dir, "visual", &chromium);
    assert_green(&output, elapsed, "visual green (committed baseline)");

    write_config(dir, "[visual]\ntest-glob = \"tests/visual-broken.spec.ts\"");
    let (output, elapsed) = run_gate(dir, "visual", &chromium);
    let doc = assert_red(&output, elapsed, "visual red (modified page vs baseline)");
    let finding = &doc["findings"][0];
    assert_eq!(finding["gate"], "visual");
    assert_eq!(finding["tool"], "playwright");
    assert_eq!(finding["rule"], "failed-spec");
    assert!(
        finding["file"]
            .as_str()
            .unwrap_or_default()
            .contains("visual-broken.spec.ts"),
        "{finding}"
    );
}

#[test]
fn a11y_gate_green_and_red_live() {
    let Some(chromium) = preconditions() else {
        return;
    };
    let tmp = setup_project();
    let dir = tmp.path();

    write_config(dir, "[a11y]\ntest-glob = \"tests/a11y.spec.ts\"");
    let (output, elapsed) = run_gate(dir, "a11y", &chromium);
    assert_green(&output, elapsed, "a11y green (accessible page)");

    write_config(dir, "[a11y]\ntest-glob = \"tests/a11y-broken.spec.ts\"");
    let (output, elapsed) = run_gate(dir, "a11y", &chromium);
    let doc = assert_red(&output, elapsed, "a11y red (seeded-issue variant)");
    let finding = &doc["findings"][0];
    assert_eq!(finding["gate"], "a11y");
    assert_eq!(finding["rule"], "failed-spec");
    assert!(finding["line"].as_u64().is_some(), "{finding}");
}

#[test]
fn perf_gate_green_and_red_live() {
    let Some(chromium) = preconditions() else {
        return;
    };
    let tmp = setup_project();
    let dir = tmp.path();

    write_config(dir, "[perf]\nlighthouse-config = \"lighthouserc.json\"");
    let (output, elapsed) = run_gate(dir, "perf", &chromium);
    assert_green(&output, elapsed, "perf green (generous budget)");

    // lhci appends to .lighthouseci between runs — start the red case clean.
    let _ = std::fs::remove_dir_all(dir.join(".lighthouseci"));
    write_config(
        dir,
        "[perf]\nlighthouse-config = \"lighthouserc-strict.json\"",
    );
    let (output, elapsed) = run_gate(dir, "perf", &chromium);
    let doc = assert_red(&output, elapsed, "perf red (absurd 1-byte budget)");
    let finding = &doc["findings"][0];
    assert_eq!(finding["gate"], "perf");
    assert_eq!(finding["tool"], "lhci");
    assert_eq!(finding["rule"], "total-byte-weight");
    assert!(
        finding["message"]
            .as_str()
            .unwrap_or_default()
            .contains("<= 1"),
        "{finding}"
    );
}
