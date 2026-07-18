# Craftsman CLI Implementation Plan

> Batched, Craftsman-style: mechanical success criteria per batch, task detail concentrated in the next 3–4 batches, revised at every boundary. Authority: docs/design/2026-07-17-cli-surface-design.md + the verification-cli research doc. This plan is executed in this repo; the CLI self-hosts its own verification from Batch 2 onward.

**Goal:** the `craftsman` binary, production grade: spec engine, verify with per-stack adapters, gate orchestration with baselines, docs pipeline, ledger commands, bootstrap — per the approved CLI surface design.

**Architecture:** single Rust binary (clap), modules not premature crates; per-stack runner adapters normalize into one six-status result schema; declarative gate adapters over hermetically pinned tools; all state writes through the CLI (single-writer).

**Tech stack:** Rust stable (2024 edition), clap, serde/toml, `gherkin` crate (cucumber-rs), quick-junit or roxmltree for JUnit parsing, `grep`/`ignore` crates for docs search, assert_cmd + cucumber-rs for the CLI's own tests.

## Global constraints (from our own conventions — this repo eats its own cooking)

- clippy `pedantic` + `nursery` warn at workspace root; CI runs `-D warnings`. No `unwrap` outside tests without a stated invariant.
- Errors: `thiserror` enums in library modules, `anyhow::Result` + `.with_context()` in `main`/command layer.
- Exit-code contract (design doc): 0 pass · 1 verification failure · 2 usage · 3 orchestrator error · 4 empty selection. Every command: `--json` (stdout) with human progress on stderr.
- Fix and refactor never share a commit; commits follow the ledger convention with trailers (hand-written until `craftsman commit` exists — Batch 3 makes the CLI take over).
- The CLI's own acceptance spec lives in `SPEC.md` (repo root), run via cucumber-rs from Batch 2 (self-hosting).

---

## Batch 0 — Repo bootstrap (this session)

Files: `.gitignore`, `cli/Cargo.toml`, `cli/src/main.rs`, `cli/rustfmt.toml`, `.github/workflows/ci.yml`, `AGENTS.md`, `craftsman.toml` (this repo's own, minimal).

- [ ] `git init`; commit the existing docs/skills corpus as the initial commit (`chore: import research corpus, design docs, and skill family`).
- [ ] `cargo init cli --name craftsman`; workspace lints per stack-rust.md (`[lints.clippy] pedantic="warn", nursery="warn"`); `main.rs` = clap skeleton with `--version` only.
- [ ] CI: fmt check, clippy `-D warnings`, `cargo test` on macOS + Linux runners.
- [ ] AGENTS.md for this repo: observed commands only (`cargo build/test/clippy`), hard constraints above, Documentation Sources table (clap → docs.rs, gherkin crate → docs.rs, cucumber-rs book).
- [ ] Success: `cargo run -- --version` prints `craftsman 0.0.1`; CI green; ledger has 2 commits.

## Batch 1 — The two spikes (parallel, throwaway dirs under `spikes/`)

Spike outcomes are ADRs, not shipped code. Each ends with a written verdict in `decisions/`.

**S1 — Gherkin → Swift Testing code-gen round trip** (`spikes/s1-swift-codegen/`):
- [ ] Hand-write `todo.feature` (2 scenarios + 1 Scenario Outline, 3 example rows).
- [ ] Hand-write the *target* generated output `TodoFeature.swift`: `@Suite` per feature, raw-identifier `@Test` function per scenario (SE-0451 backtick names = scenario names), `.tags()`, `@Test(arguments:)` for the outline; step funcs in a `Steps.swift`.
- [ ] Prove: `swift test --filter` selects one scenario by its raw-identifier name; `--parallel --xunit-output` emits the swift-testing XML; `--experimental-event-stream-output` JSONL carries per-test pass/fail with display names.
- [ ] Verdict ADR: exact name-mangling rules (spaces, unicode, uniqueness), filter regex escaping, JSONL schema version observed, xcodebuild variant deferred or confirmed.

**S2 — Result normalizer over real fixtures** (`spikes/s2-normalizer/`):
- [ ] Produce real fixture files: cucumber-json from pytest-bdd, messages NDJSON + json from @cucumber/cucumber, JUnit XML + JSONL from `swift test`, JUnit from cucumber-rs, junit from bats-core (tiny sample projects, committed as fixtures).
- [ ] Rust prototype: `enum Status { Passed, Failed, Skipped, Pending, Undefined, Ambiguous }`, `struct ScenarioResult { feature, scenario, status, steps, duration, failure }`; three-tier parser (messages → cucumber-json → junit/jsonl) each mapping fixtures into the schema; round-trip test per fixture.
- [ ] Verdict ADR: schema v1 frozen, per-runner quirks table (pytest exit 5, cucumber-rs name/tags exclusivity, swift empty-filter-exit-0 confirmed against fixtures).

Success: both ADRs written and human-reviewed; `spikes/` code stays out of `cli/`.

## Batch 2 — Spec engine + self-hosting verify (first real batch)

Files: `cli/src/{config.rs, plan.rs, spec.rs, verify/mod.rs, verify/normalize.rs (from S2), verify/adapters/cucumber_rs.rs}`, repo-root `SPEC.md` + `cli/tests/spec.rs` cucumber-rs harness.
- [x] `craftsman.toml` parsing (serde, deny_unknown_fields, versioned).
- [x] Spec engine on the `gherkin` crate: `spec status` (inventory; per-batch red/green arrives with recorded results, Batch 3+), `spec lint` (authoring rules: name uniqueness, forbidden/regex-hostile chars, batch-tag ban, missing feature name).
- [x] `verify` for the rust stack (cucumber-rs adapter): run, capture cucumber-json (output-json per ADR-003, not JUnit), normalize (S2 schema), exit-code contract incl. code 4 on empty selection.
- [x] Write this repo's SPEC.md: scenarios for `spec status/lint`, `verify` exit codes, config errors — and make them pass via the harness. **Self-hosting begun: `cargo run -- verify` green on craftsman's own spec.**
- [x] Success line: `craftsman verify` exit 0 on this repo; `spec lint` catches the seeded bad fixtures under `cli/tests/fixtures/lint/`.

Scenarios:
- Spec status lists every scenario in the spec
- Spec status emits machine-readable JSON
- Spec lint accepts a clean spec
- Spec lint rejects duplicate scenario names
- Spec lint rejects a batch tag
- Verify fails loudly when the scenario filter matches nothing
- Verify refuses to run without a craftsman config
- Config rejects a verify gate weaker than strict

## Batch 3 — Ledger + plan + doctor

- [x] `commit` (refuses on red `check-all --changed` — initially just verify+fmt+clippy as the enabled gates; writes trailers incl. `Verified-by:`), `plan lint` (batch→scenario mapping), `doctor` (config, tools, red→green round trip in a temp fixture project).
- [x] From this batch on, every commit in this repo goes through `cargo run -- commit`.
- Notes: co-author attribution is `[ledger] co-author` in craftsman.toml (committed config, one mechanism, no env var). fmt+clippy run when staged files touch the rust stack root (`[verify] cwd`); Batch 6's declarative gate adapters replace the hard-coded pair. Doctor's round-trip fixture is cached at `$TMPDIR/craftsman-doctor-fixture` (~15s cold / ~2s cached on this machine), so the round trip runs as a normal test in `cli/tests/doctor.rs` — the planned CRAFTSMAN_E2E escape hatch was not needed, and Batch 2's honest-undone exit-1 e2e gap is closed.

Scenarios:
- Plan lint accepts a plan covering existing scenarios
- Plan lint rejects a scenario missing from the spec
- Commit refuses when nothing is staged
- Commit rejects an unknown type

## Batch 4 — Python + TypeScript adapters, `--impact`

- [x] pytest-bdd adapter (cucumberjson, `-k` mapping over derived test ids, exit-5 trap, json+junit UNDEFINED merge per ADR-002) + cucumber-js adapter (messages NDJSON primary, json fallback, `--name` exact regexes, zero-scenario count → exit 4); self-contained fixture projects under `cli/tests/fixtures/{python,ts}-todo/` with pinned lockfiles, real-run integration tests unignored (~1.5s warm each).
- [x] `verify --impact [REF]`: python per-scenario coverage capture (pytest-cov test contexts → coverage-kind map, may exclude), rust/ts glue-kind maps (informational, never exclude), impact-map cache at `.craftsman/cache/impact-map.json`, conservative fallback to `--all` (loud) on missing map or git failure.
- Notes: `[verify]` became per-stack tables (`[verify.rust]` etc.) as a clean break — nothing external consumed the flat keys. JS/TS runs through bun (`bunx cucumber-js`, `bun.lock` committed), never npm/npx — bun 1.3.14 reproduced every ADR-002 cucumber-js fact. pytest-bdd's real name mangling (observed 8.1.0): spaces→`_`, other non-word chars dropped, leading digits stripped — not the plan's assumed non-alnum→underscore. A computed-empty impact set exits 0 with a loud note (coverage verdict), not exit 4 (filter typo).

Scenarios:
- Verify runs every configured stack
- Verify reports an undefined scenario as a failure
- Impact falls back to running everything when no map exists

## Batch 5 — Swift + bash code-gen adapters

- [x] `spec gen` implementing S1's ADR (Swift Testing) + bats generation: `@Suite`/raw-identifier `@Test` per scenario, outlines as `@Test(arguments:)` with typed labeled tuples, tags → generated `Tag` extension; bats with outline rows expanded as ` [row N]` tests. Single-writer split: generated runner files rewritten each run (GENERATED header), step stub templates written once, real step files never touched. Gen refuses on lint errors (exit 1), exits 4 with no code-gen stack.
- [x] `swift test` JSONL v0 parsing (event-stream pinned to version 0; testEnded symbols keyed by testID, issueRecorded texts, per-row `_testCase` display names; xunit `-swift-testing.xml` sibling as coarse fallback); ADR-001 `--filter` recipe per scenario; zero-match → exit 4 via self-count. Undefined = a failed test whose every issue carries the generated stubs' `step not implemented: ` message prefix.
- [x] bats adapter: `--formatter junit` on stdout, anchored `-f` alternation with optional row suffix, row results folded back per scenario, skip-marker → Undefined per ADR-002.
- [ ] xcodebuild/xcresulttool variant behind `[verify.swift] scheme` — **honest-undone**: the adapter refuses loudly ("not yet supported"); the xcresulttool JSON pipeline needs its own spike.
- [ ] Linux swift CI job — **honest-undone**: parked commented-out in ci.yml (setup-action coverage of Swift ≥ 6.2 on ubuntu-24.04 unverified; red CI not acceptable). bats now installs in the existing matrix, so the bash round trip runs on CI; macos-15 runs the swift round trip whenever its Xcode ships 6.2+ (the test self-skips loudly below that).
- Notes: swift round trip measured 2.9s cold / 0.7s warm (stable-path fixture cache) — far under the 90s ignore-threshold, ships unignored. Swift `#expect` failures do not abort the test, so a scenario can mix real failures with stub markers: Undefined only when ALL issues are markers. Swift Testing destructures only 2-tuples in `@Test(arguments:)` → 3+ Examples columns arrive as one labeled tuple parameter. The generated `.bats` sources `steps.bash.template` first, then `steps.bash`, so humans override stubs one function at a time.

Scenarios:
- Spec gen refuses when the spec has lint errors
- Spec gen writes a generated header
- Spec gen never overwrites step implementations

## Batch 6 — Gates, baselines, check-all

- [ ] Declarative gate adapter format (the design doc's open item #2 — designed here against the first three tools); `lint` (ruff/biome/clippy/swiftlint/shellcheck), `security` (gitleaks/semgrep/osv-scanner), hermetic tool installs into `~/.craftsman/tools/`.
- [ ] `gate baseline|strict|status`: native wraps (SwiftLint/ESLint/Semgrep) + unified snapshot for the rest; auto-ratchet + prune.
- [ ] `check-all` with file-hash cache; `mutate` (cargo-mutants/mutmut/Stryker, diff-scoped); `health` (complexity/size/duplication metrics — own implementation, thresholds in toml); `arch` v1 (import-direction rules for rust+swift via syn/swift-syntax parsing — the no-incumbent gap; `perf`/`a11y`/`visual` orchestrate lhci/axe/Playwright configs).

## Batch 7 — Docs pipeline + extract + adr

- [ ] `docs add/sync/status/search/get` (source enum from the design doc; grep-crate search; lockfile-keyed cache), `extract` (session files), `adr index/stale`.

## Batch 8 — Bootstrap + distribution + hardening

- [ ] `init` (interview scaffolding output, harness wiring templates incl. hooks + CLAUDE.md symlink), `adopt` (phase state machine), `setup` (embedded skills via include_dir + Fusion agent table), `update`; cargo-dist config + install.sh; exit-code/JSON contract test sweep across every command; `--help` audit (agent-grade help per command).
- [ ] Finish: full `check-all` green on craftsman itself, README, tagged 0.1.0 release.

---

Boundary rule: after each batch — full test suite, clippy `-D warnings`, gap check against this plan, ledger commit, revise remaining batches. Batches 4+ get task-level detail at the boundary before them, per the planning conventions.
