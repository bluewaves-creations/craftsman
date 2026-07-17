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

- [ ] `commit` (refuses on red `check-all --changed` — initially just verify+fmt+clippy as the enabled gates; writes trailers incl. `Verified-by:`), `plan lint` (batch→scenario mapping), `doctor` (config, tools, red→green round trip in a temp fixture project).
- [ ] From this batch on, every commit in this repo goes through `cargo run -- commit`.

## Batch 4 — Python + TypeScript adapters, `--impact`

- [ ] pytest-bdd adapter (cucumberjson, `-m`/`-k` mapping, exit-5 trap) + cucumber-js adapter (messages NDJSON, `--name`/`--tags`); fixture projects under `tests/fixtures/{python,ts}-todo/`.
- [ ] `verify --impact`: per-scenario coverage capture where cheap (pytest-cov contexts first), impact-map cache, conservative fallback to `--all` (loud).

## Batch 5 — Swift + bash code-gen adapters

- [ ] `spec gen` implementing S1's ADR (Swift Testing) + bats generation; `swift test` JSONL parsing on macOS, xcodebuild/xcresulttool variant behind `[verify] scheme`; Linux swift CI job.

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
