//! The write-once `XCUITest` accessibility-audit template
//! (`spec gen --a11y-stub`) for the Apple a11y path.

use std::path::Path;

use super::{FileReport, GenError, write_theirs_once};
use crate::config::Config;

/// File name of the write-once `XCUITest` audit template
/// (`spec gen --a11y-stub`), emitted at the project root.
pub const A11Y_STUB_FILE: &str = "AccessibilityAuditTests.swift.template";

/// The template's content: one test that launches the app and runs
/// `performAccessibilityAudit()` (Xcode 15+; any audit finding fails the
/// test, which the a11y gate reads from the result bundle). UI-test
/// bundles are `XCTest` even in a Swift Testing world — Swift Testing
/// does not do UI automation (research doc).
const A11Y_STUB: &str = "\
// XCUITest accessibility audit — written ONCE by `craftsman spec gen
// --a11y-stub` and never overwritten (this file is yours from now on).
// Add it to the UI-test target named in [a11y] ui-test-target (an Xcode
// target craftsman does not manage); `craftsman a11y` then runs that
// target via xcodebuild and turns failed tests into findings.

import XCTest

final class AccessibilityAuditTests: XCTestCase {
    @MainActor
    func testAccessibilityAudit() throws {
        let app = XCUIApplication()
        app.launch()
        // step: customize audit types
        // e.g. try app.performAccessibilityAudit(for: [.contrast, .hitRegion])
        try app.performAccessibilityAudit()
    }
}
";

/// `spec gen --a11y-stub` — emit the write-once audit template at the
/// project root (never overwritten once created; the file is theirs).
///
/// # Errors
/// [`GenError`] on config or write failures.
pub fn a11y_stub(cwd: &Path) -> Result<Vec<FileReport>, GenError> {
    let loaded = Config::load(cwd)?;
    let mut files = Vec::new();
    write_theirs_once(&loaded.root.join(A11Y_STUB_FILE), A11Y_STUB, &mut files)?;
    Ok(files)
}
