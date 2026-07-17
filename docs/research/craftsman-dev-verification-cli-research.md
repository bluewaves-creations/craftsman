# Building the `craftsman` CLI: Runners, Adapters & Architecture

> How to actually build the verification CLI — the per-stack Gherkin runner landscape in mid-2026, the unification layer that turns six runners into one result schema, and the language, architecture, and distribution choices for the CLI itself. All maturity claims verified against live sources on 2026-07-17 unless marked UNVERIFIED.

---

## Part 1: The Gherkin Runner Landscape (Adapter Layer)

### The Adapter Problem

`craftsman verify` must run SPEC.md scenarios on six stacks, filter by batch/tag and by scenario name, consume machine-readable results, and return exit codes. No single runner covers all stacks — the CLI is necessarily an adapter layer over per-stack runners. The evaluation criteria per stack: maintenance health, Gherkin 6 (Rule keyword) support, name/tag filtering, machine output, exit-code semantics.

### Python: pytest-bdd

| | pytest-bdd | behave | radish |
|---|---|---|---|
| Version / released | 8.1.0 / Dec 2024 (repo active thru Jul 2026) | 1.3.3 / Sep 2025 | 0.18.4 / Feb 2026 |
| Health | Active under pytest-dev; 19 months w/o release but 8.2 brewing | Revived from 1.2.6 stagnation; quiet for 10 months | Alive but marginal, still 0.x |
| Gherkin 6 (Rule) | Yes (v8, own parser subset) | Yes (1.3.x) | — |
| Name filter | `pytest -k "substring"` | `-n <regex>` | — |
| Tag filter | tags → markers: `pytest -m "batch2"` | `-t "expr"` | — |
| Machine output | `--cucumberjson=out.json` + `--junitxml` | own JSON schema (NOT cucumber-json), `--junit` | — |
| Exit codes | 0 pass / 1 fail / **5 = nothing collected** | 0 / 1 | — |

**Pick: pytest-bdd.** Cucumber JSON directly, marker filtering maps cleanly to `--batch`, and the pytest ecosystem (Hypothesis coexists as a sibling plugin in the same suite, xdist, coverage) comes free. Behave's `json` formatter is its own schema requiring conversion; radish is too small. Adapter must special-case pytest exit code 5 (empty filter = error, not success).

### TypeScript: @cucumber/cucumber, with playwright-bdd variant

- **@cucumber/cucumber v13.1.1 (Jul 2026)** — the 2024 maintainer crisis is resolved: Cucumber is "back in community ownership," funded (~$65.5k in 2025), 8 releases in 2025, v13 rewrote the parallel runtime on worker threads. Full ESM. Filtering: `--name "<regex>"`, `--tags "@batch-2 and not @wip"`. Formatters: `message` (Cucumber Messages NDJSON), `json`, `junit`, `html`.
- **playwright-bdd v9.2.0 (Jun 2026, very active)** — `bddgen` compiles `.feature` files into real Playwright tests; you get Playwright's parallelism, traces, and fixtures, plus Cucumber-standard `message`/`json` reporters. Since Craftsman's visual and a11y gates already run Playwright, a front-end project reuses one runner and one trace pipeline across `verify`, `visual`, and `a11y`.
- **vitest-cucumber v7.0.0** — parses real `.feature` files but is an authoring DSL (`describeFeature()` callbacks) with only vitest reporters. Not an adapter target.

**Pick: @cucumber/cucumber for generic TS; playwright-bdd when Playwright is in the stack.**

### Rust: cucumber crate

**cucumber-rs v0.23.0 (Apr 2026, ~16M downloads, active)** — async-first on tokio, Rule keyword supported. Cargo features `output-json` (Cucumber JSON) and `output-junit`. CLI flags `-n/--name <regex>` and `-t/--tags <expr>` — **verified caveat: they are mutually exclusive** (`conflicts_with` in cli.rs), so the adapter passes one or the other, never both. Scenario failure → nonzero exit via `run_and_exit`. cargo-nextest (0.9.140) is not Gherkin-aware and its process-per-test model fights cucumber-rs's runner; use it only for the non-BDD test remainder.

### Swift (Apple native): code-gen onto Swift Testing — no cucumber runner

The Gherkin-runner route is dead on Swift:

- **CucumberSwift 5.0.10 (Jun 2026)** — alive but low-cadence, **XCTest-only** (no Swift Testing anywhere in docs/issues), **no Linux** (platforms: iOS/macOS/tvOS only), no CLI filtering or JSON of its own.
- **XCTest-Gherkin** — dead (last commit Mar 2021).
- No maintained Gherkin-on-Swift-Testing project exists as of mid-2026; there is still no official Cucumber implementation for Swift.

The code-gen approach is feasible and verified by live experiment:

```swift
// Generated from SPEC.md by `craftsman generate`
@Suite("Todo management") struct TodoManagementFeature {
    @Test(.tags(.batch1)) func `Add a todo item`() async throws {
        // steps call shared step functions
    }
}
```

Key verified findings: SE-0451 raw identifiers (Swift 6.2+) let the scenario name BE the function name — critical because `swift test --filter` and xUnit output match the **function name, not the `@Test("...")` display name** (confirmed empirically: display-name filtering matches 0 tests). Gherkin `@tags` become `.tags(...)` traits; Scenario Outlines become `@Test(arguments:)`. On Apple, drive via `xcodebuild test -only-testing:Target/Feature/scenario` and parse JSON from `xcrun xcresulttool get test-results tests` (Xcode 16+/26 subcommands; exit code 65 is ambiguous — parse the xcresult, don't trust the code).

### Swift (Linux/SwiftPM): same generated code, `swift test`

Swift Testing is fully supported on Linux (Swift 6.3 current, March 2026). Verified `swift test` surface:

- `--filter <regex>` (target.Suite/test format), `--skip`, `swift test list` — **no tag-filter flag exists**; craftsman resolves `@tags` → scenario names itself and synthesizes the regex.
- `--xunit-output results.xml` writes two files — `results.xml` (XCTest) and `results-swift-testing.xml`. Known bugs: no file with `--disable-xctest` (SwiftPM #8000) or on zero tests (#7065).
- **Best machine path: `--experimental-event-stream-output events.jsonl`** — a documented, versioned JSON Lines ABI (swift-testing `Documentation/ABI/JSON.md`) carrying display names, tags, source locations, and pass/fail events. (Verified on macOS; Linux behavior documented as toolchain-standard but UNVERIFIED empirically.)
- Exit codes: 0 pass, 1 fail — but **a filter matching zero tests exits 0**; the adapter must treat empty matches as its own error.

### Bash: bats-core

**bats-core v1.13.0 (Nov 2025, active)** wins over shellspec (last release Jan 2021, dormant despite branch pushes into 2025). Verified flags: `-f <regex>` (name), `--filter-tags` (with `!` negation, repeatable for OR; tags via `# bats test_tags=` comments), `--formatter junit` / `--report-formatter` + `-o dir`. TAP native, JUnit for machines, no JSON. Neither tool consumes `.feature` files — craftsman generates one `.bats` file per feature, `@test "Scenario name"` per scenario, so `-f` filtering works on scenario names verbatim.

### The Unification Layer: Own the Schema, Borrow the Vocabulary

Verified emission matrix:

| Runner | Messages NDJSON | cucumber-json | JUnit XML | TAP |
|---|---|---|---|---|
| @cucumber/cucumber 13.1 | yes | yes (deprecated) | yes | — |
| playwright-bdd 9.2 | yes | yes | via PW reporter | — |
| pytest-bdd 8.1 | no | yes | yes | — |
| cucumber-rs 0.23 | no | yes (feature) | yes (feature) | — |
| swift test (6.3) | no | no | yes (+ JSONL event stream) | — |
| bats-core 1.13 | no | no | yes | yes |

**Cucumber Messages (v34.1.0, Jul 2026) is not viable as the ingestion format** — only the JS family emits it, there is no Rust library, and zero JUnit→Messages converters exist (structurally impossible: JUnit lacks pickle/step semantics). It also churns majors fast (v33→v34 within a year). The pattern the industry validates (Buildkite Test Engine ingests JUnit XML or its own JSON; Trunk Flaky Tests ingests JUnit XML, Bazel BEP, XCResult): **a small custom normalized result schema fed by per-adapter parsers**, adopting Messages' status vocabulary — `PASSED / FAILED / SKIPPED / PENDING / UNDEFINED / AMBIGUOUS` — which is a strict superset of everything JUnit/TAP can express. Three adapter tiers: Messages NDJSON (richest) → cucumber-json (step-level) → JUnit XML/JSONL (case-level floor).

For craftsman's own SPEC.md parsing (enumerate scenarios, map batches, check coverage, drive code-gen): the official cucumber/gherkin monorepo (v41, Jun 2026) has parsers for 12 languages but **not Rust** (wish-listed). The independent **`gherkin` crate by cucumber-rs (v0.16.0, Apr 2026, ~271k downloads/month, 136 dependent crates)** is actively maintained and parses real-world feature files — the right dependency for a Rust CLI.

---

## Part 2: The CLI Itself

### Implementation Language

| | Rust + clap | Go + cobra | Swift ArgumentParser | TS (Bun compile) | Python (uv) |
|---|---|---|---|---|---|
| Startup | ~5ms | ~10ms | fast | <10ms claimed | 50–150ms+ |
| Binary | 3–15 MB, musl-static | 10–30 MB | tens of MB static | **~63–116 MB** | needs runtime |
| Release automation | cargo-dist v0.32 (May 2026 — alive, NOT abandoned; Astral's fork was merged back upstream) | goreleaser v2.17 (npm publish is Pro-only) | manual / Tuist-style | bun cross-compiles 5 targets in 4s | uv publish |
| Precedent | ripgrep, uv, ruff, jj, mise, moon | gh, lefthook | Tuist (Linux via Static SDK, Feb 2026) | few | many, but slow |

Rust is the dominant 2026 pattern for exactly this class of tool, and it lets craftsman link the `gherkin` crate to parse SPEC.md natively. Go is a close second. Swift would be poetic for an Apple-centric methodology, but Tuist is the lone trailblazer and Windows/ecosystem support is weak. Bun binaries carry a ~60 MB runtime floor. Python contradicts the "fast startup, invoked hundreds of times per session" requirement.

### Prior Art: What to Steal from trunk check

Trunk sunset the Code Quality **web app** (Jul 27, 2025) but explicitly kept the CLI/IDE/CI integrations alive — the orchestrator survived; the SaaS on top didn't. Its architecture (closed-source C++ CLI behind an open bash launcher) is the closest analog to `craftsman check-all`:

1. **Version-pinning launcher** — `.trunk/trunk.yaml` pins the CLI version per repo (Gradle-wrapper pattern) → reproducible verification across humans, agents, and CI.
2. **Hermetic tool installs** — trunk downloads pinned versions of every linter itself; users never "install semgrep yourself."
3. **Declarative plugin definitions** — linters are YAML descriptions (command, parser, success codes) in an open plugins repo, each normalized to one issue schema. Adding a tool is data, not code.
4. **Hold-the-line** — only issues on new/changed lines fail. The killer brownfield-adoption feature for `lint`/`security`/`health` gates (not for `verify`, where the SPEC is absolute).
5. **File-hash caching** — skip gates whose inputs haven't changed.

Other prior art: **moon v2 "Phobos"** (May 2026, Rust, WASM-plugin toolchains) — steal task-graph + content-hash caching; **mise v2026.7.10** (Rust, extremely active) — steal the one-TOML-file DX; **just** — the simplicity baseline; **earthly** — dead (cloud shut down Jul 2025, project frozen), the cautionary tale alongside trunk's pivot: keep craftsman's value in the free CLI, never gate verification behind a service.

### Architecture: Gates × Adapters, One Config

```
craftsman (Rust binary)
├── SPEC engine: gherkin crate → scenario inventory, batch map, coverage
├── Gate: verify ──► runner adapter (per stack)
│     python: pytest-bdd     ts: cucumber-js | playwright-bdd
│     rust: cucumber-rs      swift: codegen + swift test / xcodebuild
│     bash: codegen + bats-core
├── Gates: lint | arch | security | health | perf | a11y | visual
│     each = declarative adapter over external tools
│     (ruff/eslint/clippy, dependency-cruiser/import-linter,
│      semgrep + gitleaks + osv-scanner, lighthouse-ci, axe, playwright)
├── Normalizer: every adapter → one result schema (Messages status vocab)
└── check-all: gate orchestration + file-hash cache + budgets
```

```toml
# craftsman.toml — auto-detected defaults, explicit overrides
[project]
stack = "swift-apple"          # auto-detected from Package.swift/pyproject/etc.
spec = "SPEC.md"

[verify]
runner = "swift-testing-codegen"
scheme = "MyApp"               # xcodebuild only

[gates]
enabled = ["verify", "lint", "arch", "security", "health"]

[gates.security]
tools = { secrets = "gitleaks@8", sast = "semgrep@1", sca = "osv-scanner@2" }

[gates.perf]
budget.p95_ms = 200
```

Auto-detection (à la trunk init/ruff) proposes; craftsman.toml disposes — the file is committed so the agent, the human, and CI run identical gates. The CLI orchestrates and normalizes; it never reimplements a linter or scanner.

### Output Contract: Designed for Agents First

2026 findings converge (a 75-test study found CLI agents 10–32x cheaper in tokens and ~100% vs 72% reliability against MCP equivalents — "build a good CLI first, then wrap as MCP"; gh CLI and clig.dev supply the conventions):

- `--json` on every command; **JSON to stdout, human progress to stderr**; JSON Lines streaming for per-scenario/per-gate verdicts as they finish.
- Semantic exit codes as a documented contract: `0` all green, `1` verification failed (scenarios red / gate over budget), `2` usage error, `3` orchestrator error (tool missing, config invalid), and — learned from pytest's 5 and swift test's empty-filter-exits-0 — **a distinct code (4) for "filter matched no scenarios"**, never silent success.
- `--help` is the agent's documentation surface: exhaustive, example-rich, noun-verb grammar, `--json` documented per command.
- Non-interactive by default (TTY detection, no prompts, no pager, no color in pipes); machine-readable error codes (`"tool_not_found"`) with recovery suggestions; idempotent commands.

```json
{"gate":"verify","status":"failed","batch":2,"scenarios":{"passed":11,"failed":1,"undefined":0},
 "failures":[{"scenario":"Add a todo item","step":"Then the todo list contains \"Buy milk\"",
              "file":"SPEC.md:42","message":"expected 1 item, found 0"}]}
```

### Distribution

The 2026 standard stack for a Rust CLI, all generated by cargo-dist from one config: GitHub Releases + `curl|sh` installer + Homebrew tap (core later, if ever) + cargo-binstall + npm wrapper package (platform binaries via optionalDependencies — the esbuild/Biome pattern). Add aqua-registry entry so `mise use -g craftsman` works (mise's `aqua:` backend is the recommended path; `ubi:` is deprecated in favor of `github:`) — exactly how Tuist ships. `craftsman self-update` via cargo-dist's axoupdater; per-repo version pinning in craftsman.toml (trunk's launcher lesson) for reproducibility. uvx/pip only as a bonus channel, if ever.

---

## What Craftsman Dev Should Adopt

1. **Rust + clap for the CLI** — matches the ripgrep/uv/ruff/mise/moon precedent, ~5ms startup for a tool invoked hundreds of times per agent session, single static binary, and native access to the `gherkin` crate for SPEC.md parsing.
2. **Per-stack runner adapters, not one runner**: pytest-bdd (Python), @cucumber/cucumber with a playwright-bdd variant (TS), cucumber-rs (Rust), **code-gen onto Swift Testing with raw-identifier test names** (both Swifts), code-gen onto bats-core (bash).
3. **Own normalized result schema with Cucumber Messages' status vocabulary** (PASSED/FAILED/SKIPPED/PENDING/UNDEFINED/AMBIGUOUS), fed by tiered parsers: Messages NDJSON → cucumber-json → JUnit XML/JSONL event stream. UNDEFINED is load-bearing: it's how `craftsman verify` reports "scenario exists in SPEC.md but has no step implementation" as a distinct failure mode.
4. **Trunk-check architecture**: declarative tool adapters, hermetic pinned tool installs, per-repo CLI version pinning, file-hash caching, hold-the-line for advisory gates (lint/security/health) — never for `verify`.
5. **Agent-first output contract**: `--json` everywhere, stdout/stderr discipline, five-value exit-code contract including "empty filter" as an explicit error, help text written as agent documentation.
6. **cargo-dist distribution** with Homebrew tap, curl|sh, npm wrapper, cargo-binstall, and aqua/mise registry.

## What NOT to Adopt

- **Cucumber Messages as the ingestion/interchange format** — only JS-family runners emit it, no Rust library, no JUnit→Messages path, fast major-version churn. Borrow its vocabulary; own the schema.
- **CucumberSwift or any Swift Gherkin runner** — XCTest-only, no Linux, near-dormant. Code-gen is strictly better and Swift-Testing-native.
- **A cucumber runner requirement at all for Swift/bash** — mapping scenarios to natively-named tests keeps `swift test --filter` and `bats -f` working with zero runner dependency.
- **behave, radish, vitest-cucumber, shellspec, XCTest-Gherkin** as adapter targets — maintenance risk or non-standard output.
- **Writing the CLI in TypeScript/Bun (~60–116 MB binaries), Python (startup), or Swift (pioneer tax)** despite the Apple affinity.
- **Reimplementing any underlying tool** — craftsman orchestrates semgrep/gitleaks/axe/lighthouse/dependency-cruiser; it never forks them.
- **A SaaS layer** — trunk's web-app sunset and earthly's death (both Jul 2025) show the orchestrator CLI is the durable artifact; verification must never require a service.

## Conclusion

The recommended architecture: **a single Rust binary** (clap; released via cargo-dist to Homebrew tap, curl|sh, npm, cargo-binstall, and aqua/mise) with **`craftsman.toml`** as the committed contract (auto-detected, explicitly overridable, CLI-version-pinned). Inside: a **SPEC engine** on the cucumber-rs `gherkin` crate that enumerates scenarios and maps batches; a **runner adapter per stack** — pytest-bdd, @cucumber/cucumber or playwright-bdd, cucumber-rs, generated Swift Testing suites driven by `swift test`/`xcodebuild`, generated bats files — each translating `--batch`/`--scenario` into the runner's native tag/name/filter flags (with the verified traps handled: pytest exit 5, cucumber-rs name/tag exclusivity, swift test's empty-filter success, xcodebuild's ambiguous 65); a **normalizer** collapsing Messages/cucumber-json/JUnit/JSONL into one six-status result schema; and a **gate orchestrator** (`check-all`) that runs verify/lint/arch/security/health/perf/a11y/visual as declarative adapters over pinned external tools, with caching and budgets.

This is the machine actor made concrete: every gate mechanical, every result one schema, every answer an exit code. The two builds to prototype first are the highest-risk adapters — the Swift Testing code generator (raw-identifier round-tripping through `--filter` and the JSONL event stream) and the normalizer's three-tier parser — because everything else is assembly of verified, healthy parts.
