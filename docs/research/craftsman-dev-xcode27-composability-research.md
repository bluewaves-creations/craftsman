# Xcode 26/27-Era Agentic Tooling: Composability Research

> How Craftsman Dev composes with Apple's agentic toolchain — Xcode 27's bundled Agent Skills, the MCP bridge, Device Hub, and the CLI verification surface — evaluated against what actually shipped as of July 2026, with confirmed facts separated from unverified claims.

> **Correction (2026-07-18, verified on Xcode 27 GA on this machine):** the export command is `xcrun mcpbridge run-agent skills export [--output-dir] [--replace-existing]`, and two skills ship under different names than beta coverage reported: `modernize-tests` (not `test-modernizer`) and `adopt-c-bounds-safety` (not `c-bounds-safety`). Everything else in the skills table held.

---

## The Question

The methodology doc asserts that on Apple platforms Craftsman Dev composes with Xcode 27's bundled skills (`test-modernizer`, `swiftui-specialist`, `device-interaction`) and "Device Hub" rather than reimplementing platform idiom. Is that real? What exactly shipped in the Xcode 26 → 27 cycle, which pieces work headless for mechanical verification, and where is the precise line between what Craftsman OWNS and what it DELEGATES?

## Verification Status Summary

| Claim in methodology doc | Status |
|---|---|
| Xcode 27 exists, announced WWDC 2026 (June 2026) | **CONFIRMED** ([MacRumors](https://www.macrumors.com/2026/06/08/wwdc-2026-recap/), [Appcircle](https://appcircle.io/blog/wwdc-2026-recap-for-developers)) |
| Xcode 27 bundles Apple-written Agent Skills | **CONFIRMED** — seven skills ([DEV](https://dev.to/arshtechpro/wwdc-2026-xcode-27-ships-with-apples-own-agent-skills-what-they-are-and-how-to-use-them-3g2), [SwiftLee](https://www.avanderlee.com/ai-development/using-xcode-27s-agent-skills-in-claude-codex-and-cursor/)) |
| `test-modernizer` (XCTest → Swift Testing migration) | **CONFIRMED**, exact name |
| `swiftui-specialist` (SwiftUI conventions) | **CONFIRMED**, exact name |
| `device-interaction` (screenshots, UI hierarchy, synthesized touch) | **CONFIRMED**, exact name — but **only functions inside Xcode 27**, not portable |
| "Device Hub" | **CONFIRMED** — redesigned unified simulator + physical device management in Xcode 27 |
| MCP support in Xcode | **CONFIRMED** — `xcrun mcpbridge`, shipped in Xcode 26.3 (~Feb 2026), extended in 27 |
| Agents runnable headless via Xcode's own tooling | **NOT CONFIRMED** — `mcpbridge` requires a live Xcode GUI process; no evidence of a headless `xcrun agent run` |

Unverified details flagged inline below: the exact Xcode 26.3 GA date, Device Hub internals, and whether `device-interaction`'s tools are reachable through `mcpbridge` (as of beta coverage, they are not among the 20 exposed tools).

## The Native Stack: What Shipped When (CONFIRMED)

**WWDC 2025 → Xcode 26.** "Coding Intelligence": built-in ChatGPT, later GPT-5 and native Claude (Sonnet 4) account integration in the Intelligence settings panel, plus API-key providers and local models on Apple silicon ([MacRumors](https://www.macrumors.com/2025/08/28/xcode-gpt-5-claude-integration/), [9to5Mac](https://9to5mac.com/2025/08/28/new-xcode-beta-now-available-with-gpt-5-and-claude-support/)). Chat-assistant era: no skills, no MCP.

**Xcode 26.3 (early 2026).** The pivot to agents. Two protocols ([DEV](https://dev.to/arshtechpro/xcode-263-use-ai-agents-from-cursor-claude-code-beyond-4dmi), Apple's [Giving external agents access to Xcode](https://developer.apple.com/documentation/xcode/giving-external-agents-access-to-xcode)):

- **ACP (Agent Client Protocol)** — external agents (Claude Code, Codex, Cursor, Gemini) run *inside* Xcode's UI as the coding assistant.
- **MCP via `xcrun mcpbridge`** — external CLI agents reach *into* Xcode: `claude mcp add --transport stdio xcode -- xcrun mcpbridge`.

`mcpbridge` translates MCP-over-stdio into XPC calls against the running Xcode process and exposes ~20 typed tools ([Daniel Vaughan](https://codex.danielvaughan.com/2026/06/11/xcode-27-codex-cli-mcp-bridge-apple-agentic-coding-ios-macos-development/)): file ops (`XcodeRead`/`XcodeWrite`/`XcodeGrep`/…), **`BuildProject`** (typed error objects with file/line/severity — not free text), `GetBuildLog`, **`RunAllTests`/`RunSomeTests`/`GetTestList`**, navigator diagnostics, `ExecuteSnippet` (Swift REPL), **`RenderPreview`** (headless SwiftUI preview rendering), `DocumentationSearch`. Hard limitations: **Xcode must be running (GUI, frontmost project), macOS only** — it is an IDE bridge, not a CI surface.

**WWDC 2026 → Xcode 27.** On-device AI code completion (Neural Engine, code stays local; agentic multi-step work still routes to Anthropic/OpenAI/Google by opt-in), agentic workflows that plan features, run tests, drive simulators, analyze crashes, and propose fixes; AI-powered localization; a **redesigned Device Hub** consolidating simulator + physical device management; and the seven bundled Agent Skills ([byteiota](https://byteiota.com/xcode-27-agentic-coding-mcp-guide/), [Appcircle](https://appcircle.io/blog/wwdc-2026-recap-for-developers)).

## The Seven Bundled Skills (CONFIRMED)

Exported via the toolchain — they follow the open Agent Skills format (`SKILL.md` + supporting files):

```bash
xcrun agent skills export --output-dir ~/.agents/skills   # shared discovery dir for Claude/Codex/Cursor
```

| Skill | Purpose | Portable outside Xcode? |
|---|---|---|
| `swiftui-specialist` | Apple-written idiomatic SwiftUI patterns and review guidance | Yes — knowledge only |
| `swiftui-whats-new-27` | This cycle's new SwiftUI APIs and deprecations | Yes |
| `test-modernizer` | XCTest → Swift Testing migration practice | Yes |
| `uikit-app-modernization` | Modernizing older UIKit (multi-window etc.) | Yes |
| `c-bounds-safety` | Adopting C bounds-safety extensions | Yes |
| `audit-xcode-security-settings` | Project security configuration review | Degrades — falls back to manual `.pbxproj` editing |
| `device-interaction` | Verify behavior on simulator/device: screenshots, UI hierarchy inspection, synthesized touch; runs as a subagent | **No** — depends on tools only the Xcode 27 agent provides |

The critical composability fact ([SwiftLee](https://www.avanderlee.com/ai-development/using-xcode-27s-agent-skills-in-claude-codex-and-cursor/)): knowledge skills travel anywhere; `device-interaction` does not. "Outside of Xcode there is simply nothing to call." For a CLI-first methodology, the `device-interaction` capability must be reproduced by third-party tooling (AXe / XcodeBuildMCP, below) — the *skill* cannot be delegated to headlessly.

## CLI-First Verification Surface (what `craftsman verify` shells out to)

All of the following run without Xcode's GUI — genuine machine-actor territory.

### Test execution and result parsing (CONFIRMED)

```bash
# Xcode projects — exit code is the gate; the bundle is the evidence
xcodebuild test -scheme App -destination 'platform=iOS Simulator,name=iPhone 17' \
  -testPlan Batch3 -resultBundlePath out.xcresult

# Structured JSON since Xcode 16 (the old `get object` needs --legacy and is dying)
xcrun xcresulttool get test-results summary --path out.xcresult   # also: tests, test-details, activities, insights

# SwiftPM (macOS and Linux) — filtering + JUnit XML
swift test --filter "SpecScenarios" --parallel --xunit-output report.xml
```

Gotchas verified against SwiftPM issues: `--xunit-output` requires `--parallel` to emit reliably; Swift Testing results land in a sibling `report-swift-testing.xml`; `--disable-xctest` + `--xunit-output` was broken ([swiftlang/swift-package-manager#8000](https://github.com/swiftlang/swift-package-manager/issues/8000)) — pin behavior in CI. `xcresulttool get test-results` emits undocumented-but-stable JSON with `summary`/`tests`/`insights` subcommands ([Apple forums](https://developer.apple.com/forums/thread/763888)).

### Swift Testing state, mid-2026 (CONFIRMED)

Swift Testing is the default; WWDC26 shipped a dedicated [Migrate to Swift Testing](https://developer.apple.com/videos/play/wwdc2026/267/) session (the human-facing twin of `test-modernizer`). Current capabilities ([What's new in Swift, WWDC26](https://developer.apple.com/videos/play/wwdc2026/262/)):

- **Parameterized tests**: `@Test(arguments:)` — each argument combination is a separately reportable, separately re-runnable test, run in parallel. This is the Gherkin `Scenario Outline` primitive.
- `#expect` / `#require` macros with expression capture.
- **Exit tests**: `#expect(processExitsWith:)` runs the body in a child process — macOS/Linux/FreeBSD/Windows only (not iOS). Perfect for CLI-tool specs.
- **Attachments**: arbitrary `Attachable` data on results — including images on Apple platforms (Swift 6.3/6.4 era) — screenshots ride inside the xcresult.
- Swift 6.3/6.4 additions: `.warning` issue severity (soft failures), `Test.cancel`, XCTest interop in both directions.

### Simulator and visual verification (CONFIRMED, long-stable)

```bash
xcrun simctl boot "iPhone 17" && xcrun simctl io booted screenshot shot.png
xcrun simctl io booted recordVideo run.mp4
xcrun simctl status_bar booted override --time "9:41" --batteryLevel 100   # deterministic screenshots
xcrun simctl ui booted appearance dark
```

`simctl` gives screenshots/video/appearance headlessly but **no touch synthesis and no UI hierarchy** — that gap is exactly what Device Hub + `device-interaction` fill inside Xcode, and what AXe fills on the CLI.

### Accessibility audit (CONFIRMED)

`try app.performAccessibilityAudit()` in an XCUITest (Xcode 15+, iOS 17+ runtimes) audits contrast, hit region, dynamic type, element description, clipped text; any finding fails the test; runs under `xcodebuild test` in CI ([Apple docs](https://developer.apple.com/documentation/xctest/xcuiapplication/4191487-performaccessibilityaudit), [polpiella.dev](https://www.polpiella.dev/xcode-15-automated-accessibility-audits/)). Note it lives in XCTest's UI-testing side — UI test bundles remain XCTest even in a Swift Testing world (Swift Testing does not do UI automation).

### Headless matrix

| Capability | Headless CI? | Tool |
|---|---|---|
| Build + unit/UI tests, exit code | Yes | `xcodebuild test`, `swift test` |
| Structured result JSON | Yes | `xcresulttool get test-results` |
| Screenshots / video | Yes | `simctl io` |
| Touch synthesis + UI hierarchy | Yes, third-party | AXe / XcodeBuildMCP (beta) |
| Accessibility audit | Yes | `performAccessibilityAudit` via `xcodebuild` |
| Typed build diagnostics, `RenderPreview`, REPL | **No** — needs running Xcode | `xcrun mcpbridge` |
| `device-interaction` skill | **No** — Xcode 27 agent only | — |

## Third-Party Landscape (mid-2026 health check)

| Tool | What | Health |
|---|---|---|
| [XcodeBuildMCP](https://github.com/cameroncooke/XcodeBuildMCP) | MCP server over `xcodebuild`/`simctl`; ~70 tools; build, test, log capture, UI automation (beta) with UI snapshots, stable element refs, screen hashes | **Healthy** — now stewarded under the [getsentry org](https://github.com/getsentry/XcodeBuildMCP); works with no Xcode instance running |
| [AXe](https://github.com/cameroncooke/AXe) | CLI simulator automation via private Accessibility APIs: `axe describe-ui`, `axe tap --label`, screenshots, video, gesture presets | **Healthy** — the headless `device-interaction` analog |
| xcodemake | Incremental builds by converting xcodebuild logs to Makefiles | Alive as XcodeBuildMCP's incremental-build engine (with auto-fallback to `xcodebuild`) |
| Sweetpad | VS Code/Cursor Xcode-less development UX | Alive; user-driven, not agent/gate tooling |
| [swift-snapshot-testing](https://github.com/pointfreeco/swift-snapshot-testing) (Point-Free) | Snapshot/visual regression: views, images, custom dumps, inline snapshots | **Healthy** — v1.19.3, updated July 2026; native Swift Testing support since 1.17 |
| [CucumberSwift](https://github.com/Tyler-Keith-Thompson/CucumberSwift) | Runtime Gherkin interpreter for Swift | **Stale-ish** — XCTest-bound, sparse releases; no Swift Testing story found |

swift-snapshot-testing is the Apple-platform analog of the Playwright screenshot gate from the front-end research: baselines in git, pixel comparison, mechanical pass/fail, runs under `swift test`.

## Gherkin → Swift Testing Mapping

Two options for executing SPEC.md scenarios on Apple platforms:

**Option A — CucumberSwift (runtime interpretation).** Parses `.feature` files at test time. Rejected: XCTest-bound (fights the `test-modernizer` direction), effectively unmaintained, step-to-scenario mapping is dynamic so `--filter`/test-plan selection of individual scenarios is weak.

**Option B — code-gen to parameterized Swift Testing (recommended).** `craftsman` generates one `@Test` per scenario (or one parameterized `@Test(arguments:)` per Scenario Outline) from SPEC.md, tagged for batch selection:

```swift
extension Tag { @Tag static var batch3: Self; @Tag static var checkout: Self }

@Suite("Feature: Checkout") struct CheckoutSpec {
    @Test("Scenario: Discount applied", .tags(.checkout, .batch3),
          arguments: [(3, 0.0), (5, 0.10), (10, 0.15)])   // Examples table → arguments
    func discountApplied(qty: Int, discount: Double) async throws {
        let cart = Cart(quantity: qty)                     // Given
        let total = cart.checkout()                        // When
        #expect(total.discount == discount)                // Then
    }
}
```

Selection composes with both runners: `swift test --filter CheckoutSpec` on SwiftPM/Linux; `.xctestplan` per batch (or `-only-testing:`) under `xcodebuild`. Each Examples row is individually reported and re-runnable; failures carry `#expect` expression captures; screenshots attach via Attachments. Generated files are named `SPEC.md`-traceably (scenario name in the `@Test` display name) so the xcresult/JUnit report reads as the living spec. This is exactly the pattern already used for pytest-bdd elsewhere in the methodology: Gherkin is the source of truth, the test file is generated glue.

## Composability: What Craftsman OWNS vs DELEGATES

| Responsibility | Owner | Rationale |
|---|---|---|
| SPEC.md Gherkin authoring + scenario → `@Test` code-gen | **Craftsman** | Agent-agnostic core of the methodology |
| Batch gating: run tests, parse exit codes + `xcresulttool`/JUnit JSON | **Craftsman CLI** | Mechanical verification is the product |
| Snapshot/visual gate orchestration (baselines, diff policy) | **Craftsman CLI** (delegating pixels to swift-snapshot-testing) | Same gate shape as Playwright on web |
| Simulator driving in CI (screenshots, taps, hierarchy) | **Delegate**: `simctl` + AXe (or XcodeBuildMCP when the agent is MCP-capable) | Never reimplement HID/accessibility plumbing |
| Swift/SwiftUI idiom, XCTest migration knowledge | **Delegate**: exported Apple skills (`swiftui-specialist`, `test-modernizer`, `swiftui-whats-new-27`) | Apple's docs-team-written skills ARE the librarian's official documentation |
| In-IDE typed diagnostics, preview rendering | **Delegate**: `xcrun mcpbridge` when a human has Xcode open | Bonus surface, never a gate dependency |
| Device Hub / `device-interaction` | **Delegate, optional** | Xcode-27-only; the CLI path must not require it |

Portability rule for the agent-agnostic skill: reference Apple capabilities **by probe, not by assumption** — "if `xcrun agent skills export` succeeds, install Apple's skills alongside this one and defer to them on SwiftUI/testing idiom; if AXe is present, use it for simulator interaction; otherwise fall back to `simctl` screenshots only." The skill names commands and exit codes, never Xcode UI.

## What Craftsman Dev Should Adopt

1. **Export-and-defer for Apple's knowledge skills** — bootstrap step runs `xcrun agent skills export` into the agent's skills directory; Craftsman's Apple skill defers to `swiftui-specialist`/`test-modernizer` for idiom rather than restating it.
2. **`xcresulttool get test-results` JSON as the canonical Apple gate evidence** (with `--legacy` shims for pre-16 toolchains only if a target demands it).
3. **Code-gen Gherkin → parameterized Swift Testing** with Tags + `.xctestplan`/`--filter` for batch selection; identical shape on macOS and Linux SwiftPM.
4. **swift-snapshot-testing as the visual gate**; simctl `status_bar override` for deterministic full-app screenshots.
5. **AXe as the headless `device-interaction` equivalent** for agent-driven simulator smoke checks; XcodeBuildMCP as the MCP-native alternative for agents that speak MCP.
6. **`performAccessibilityAudit` as a non-negotiable UI-test gate** — the axe-core analog on Apple platforms.
7. **`mcpbridge` as an opportunistic enhancement** documented for interactive sessions (typed diagnostics, `RenderPreview`), never required by `craftsman verify`.

## What NOT to Adopt

**Depending on Xcode's agent for verification.** `mcpbridge` and `device-interaction` require a live, frontmost Xcode GUI. A gate that needs an IDE window open is not mechanical verification. CI truth stays with `xcodebuild`/`swift test` exit codes.

**Reimplementing Apple's skills content.** Apple now ships and maintains SwiftUI/testing idiom as skills in the open Agent Skills format. Duplicating that content guarantees drift; Craftsman's Apple skill should be thin orchestration plus references.

**Runtime Gherkin interpretation (CucumberSwift).** XCTest-bound and stale against a platform actively migrating to Swift Testing; code-gen keeps the spec authoritative without inheriting the dependency.

**Private-API tooling as a hard dependency.** AXe rides private Accessibility APIs — excellent for local agent loops, but gates must degrade gracefully to `simctl` + snapshot tests when it breaks against a new runtime.

## Conclusion

The methodology doc's claims hold up: Xcode 27 (WWDC 2026) really ships `test-modernizer`, `swiftui-specialist`, `device-interaction`, and Device Hub, and the skills are exportable in the open Agent Skills format — Apple effectively endorsed the composability bet. But the confirmed details sharpen the design: Apple's agentic surface splits into a **portable knowledge layer** (exported skills — delegate to it), an **IDE-bound interaction layer** (`mcpbridge`, `device-interaction`, Device Hub — enhance with it, never depend on it), and a **headless verification layer** (`xcodebuild`/`swift test`/`xcresulttool`/`simctl` plus third-party AXe and swift-snapshot-testing — build the gates on it). Craftsman owns the spec, the code-gen, and the gate orchestration; Apple owns the idiom and the devices; the machine's exit codes remain the only opinion that counts.
