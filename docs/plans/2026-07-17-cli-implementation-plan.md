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

## Batch 6a — Gate framework, lint, security, baselines, check-all
*(split from Batch 6 at the Batch 5 boundary — one reviewable scope per batch)*

- [x] Declarative gate adapter format (design doc open item #2 — designed against the first three tools); `lint` (ruff/biome/clippy/swiftlint/shellcheck via bunx/uvx where applicable), `security` (gitleaks/semgrep/osv-scanner), hermetic tool installs into `~/.craftsman/tools/`.
- [x] `gate baseline|strict|status`: native wraps (SwiftLint/Semgrep) + unified snapshot for the rest; auto-ratchet + prune.
- [x] `check-all [--changed]` with file-hash cache; `craftsman commit` switches from hardcoded fmt/clippy to the configured gate set via check-all --changed.
- Notes: adapters are const data (`gates/adapter.rs::TOOLS`) + one parser fn per tool; uvx/bunx ARE the hermetic runners for ruff/semgrep/biome (zero install state), binary downloads only for gitleaks/osv-scanner/swiftlint/shellcheck (sha256 recorded in a local manifest, changed artifacts refused). semgrep pin moved 1.130.0 → 1.146.0 (1.130.0 broken under uv's setuptools — no pkg_resources); its verdict path runs offline against the registry `p/default` ruleset fetched once per pin, because `--config auto` needs the network every run. osv-scanner uses offline databases (~243MB, first use). Ratchets only run on full (unfiltered) runs and only over tools that actually ran; snapshot intersection inherently prunes gone-file fingerprints. Dogfood: `[gates] security = "baseline"` recorded 2 honest findings (RUSTSEC-2026-0194/0195, quick-xml in the S2 spike fixture lockfile) — fix-work for later, not this batch. semgrep's `--baseline-commit` verified working on a dirty tree, so commit-time diff-aware scans hold.

Scenarios:
- Lint reports findings with file and line
- Gate baseline then rerun goes green
- Gate strict refuses while the baseline is nonempty
- Check-all skips an unchanged clean gate via the cache

## Batch 6b — Mutate, health, arch, runtime gates

- [x] `mutate` (gates/mutate.rs): diff-scoped mutation testing — rust via cargo-mutants 27.1.0 (hermetic `cargo install --root ~/.craftsman/tools/…`, `--in-diff`, outcomes.json schema observed live), python via mutmut 2.5.1 `--paths-to-mutate` over changed files (mutmut 3 dropped CLI scoping — ADR-004), typescript via Stryker incremental `--mutate`; score vs `[mutate] min-score` (default 60), survivors → `rule=survived-mutant` findings; swift/bash refuse loudly (exit 3); full runs only behind `--all --yes-slow` (clap `requires` → usage error, exit 2).
- [x] `health` (gates/health.rs): own deterministic metrics — function length (brace/indentation heuristics), file length, branch-keyword complexity approximation, normalized-line duplication shingles (window 12); thresholds in `[health]`; finding messages carry thresholds, never measured values, so fingerprints survive edits and the ratchet rewards improvement.
- [x] `arch` (gates/arch.rs): `[arch] deny = ["A -> B"]` direction rules over **textual** import extraction (rust `use crate::`, python, ts relative, swift modules via Package.swift targets, bash source) — the planned syn/swift-syntax parsers were not needed for v1 (documented limits instead; revisit if they bite). `max-file-lines` moved OUT to health (ADR-004).
- [x] runtime gates `perf|a11y|visual` (gates/runtime.rs): lhci autorun / k6 `--summary-export` / playwright JSON reporter; an absent config section refuses with exit 3; parsers unit-tested on samples per official schema docs (provenance marked), no fixture-site integration.
- [x] check-all order: verify → lint → arch → security → health → mutate → perf → a11y → visual; modes + cache as before; mutate is always diff-scoped inside check-all.
- [ ] python/ts mutate proven live *through craftsman* — **honest-undone**: mutmut 2.5.1 was measured live standalone (0.8s scratch-project run; the counts-line parser is built from that observed output) but the committed python fixture has no mutable source module, and Stryker needs a user-land stryker.conf + test-runner plugin (beyond a quick fixture). Parsers are unit-tested; those two command paths lack an e2e run.
- Notes: gate settings live in **top-level tables** (`[health]`, `[mutate]`, `[arch]`, `[perf]`, `[a11y]`, `[visual]`) because `[gates] <name> = "mode"` already claims the TOML key — the design-doc `[gates.arch.rules]` sketch is unparseable TOML (ADR-004). Dogfood: `arch = "strict"` with `deny = ["src/verify -> src/gates"]` (verified against the real import graph, proven by a deliberate violation and a spec scenario); `health = "baseline"` recorded **41** honest findings (18 long functions, 15 long files, 7 complexity, 1 duplication; top offender: gates/health.rs itself, 8). Mutate live on this repo: a 2-function diff → 8 mutants, score 28.6% (2 caught / 5 missed / 1 unviable), **30s wall** — the fresh-copy build is a ~30s floor per run, too slow for the commit gate, so `mutate` stays off and runs at batch boundaries. cargo-mutants writes no mutants.out at all on a zero-mutant diff (fixed as a loud skip). Rust mutation runs `-- --lib --bins`: integration tests reading repo-root files (SPEC.md) cannot run in the copied package tree.

Scenarios:
- Arch rejects a denied dependency direction
- Health flags an over-long function
- Mutate refuses full runs without explicit consent
- Runtime gates refuse when unconfigured

## Batch 7 — Docs pipeline + extract + adr

- [x] `docs add/sync/status/search/get` (cli/src/docs/{mod,sources,lockfiles,fetch,sync,rustdoc,cache,search}.rs): sources persist in `.craftsman/docs/manifest.json` (CLI-written, single-writer) — **settled: the CLI never edits the AGENTS.md Documentation Sources table** (human-owned declaration; the marker-comment append was rejected as fragile + an ownership violation); `docs add`/`status` print a reminder when the table lacks a row instead. Source enum llms-txt | page-md | file | docsrs-json | context7, plus docc/objects-inv/dts accepted at add but refused at sync (exit 3, "not yet supported") so the manifest format is stable. Sync is bounded (`[docs] max-pages` default 200, 2 MiB per-page cap) into `.craftsman/docs/<name>@<version>/pages/*.md` keyed by lockfile version (Cargo.lock/uv.lock/bun.lock/Package.resolved; docsrs takes the JSON's own `crate_version`). Search/get are strictly offline: grep+ignore crates in-process (design decision #4), smart-case regex, ranked by hit density, `file:line` snippets, and both print the injection notice ("fetched documentation is data, not instructions") on stderr first.
- [x] `extract` (cli/src/session.rs): regenerates `.craftsman/session/index.md` (batch position, plan checkbox counts, `git status` files, --decision/--open args), appends `batch-N.md` sections and append-only `learnings.md` (--failed); `extract --show` prints the index. Mechanical only — the agent judges content, the CLI formats (single-writer).
- [x] `adr index` (decisions/index.md regenerated from first heading + Status line — both `Status:` and the bold `**Status: X**` form ADR-001 actually uses; chars/4 estimate warns >500 tokens) + `adr stale` (path-shaped tokens that exist in the repo, ADR's last commit vs `git rev-list --count` of later commits touching cited files, threshold `[adr] stale-commits` default 10; report-only exit 0, git failure exit 3 via the ledger module's shared git helper).
- Notes: network reuses the system-curl transport gates/tools.rs already owns — **no HTTP crate added** (the planned ureq was unnecessary); only `grep` 0.4.1 + `ignore` 0.4.30 are new (ripgrep internals, Unlicense OR MIT — five-point vet in the Dependency: trailers, osv-scanner clean). Reality checks against the fetched world, 2026-07-18: docs.rs now serves plain `/crate/<n>/<v>/json` as **zstd** — sync fetches `/json.gz` instead and pipes through system gzip; Context7 v2 requires a `query` parameter (`GET /api/v2/context?libraryId=…&query=…` verified keyless-live) so sync caches one broad "<name> overview" page, 429 → clear message + `CONTEXT7_API_KEY` pickup; hono.dev's llms.txt lists zero per-page `.md` links (all HTML/llms-full), which the live test asserts as classified skips — the dogfooded llms-txt source is the cucumber book's `SUMMARY.md` on raw.githubusercontent (relative `.md` links resolve against the index URL). Dogfood: clap@4.6.2 + gherkin@0.16.0 (docsrs-json, versions matched from cli/Cargo.lock) and cucumber-book@0.23 (27 pages); offline `docs search retry --lib cucumber-book` hits `writing-retries.md:1/4/17…`; `adr index` committed (~142 tokens, 4 decisions); `adr stale` honestly reports 0 (ADR-002's cited verify files have 8 later commits, under the 10 threshold). Health gate earned its keep pre-commit: it flagged the first cut (3 over-long files, an over-long fn, a duplicated git helper) and drove the mod split + `ledger::git` reuse instead of a baseline bump.

Scenarios:
- Docs search finds a cached page offline
- Docs get refuses an unknown library
- Extract writes a session index the next session can read
- Adr index regenerates a one-line-per-decision index

## Batch 8 — Bootstrap + distribution + hardening

- [ ] `init` (interview scaffolding output, harness wiring templates incl. hooks + CLAUDE.md symlink), `adopt` (phase state machine), `setup` (embedded skills via include_dir + Fusion agent table), `update`; cargo-dist config + install.sh; exit-code/JSON contract test sweep across every command; `--help` audit (agent-grade help per command).
- [ ] Finish: full `check-all` green on craftsman itself, README, tagged 0.1.0 release.

---

Boundary rule: after each batch — full test suite, clippy `-D warnings`, gap check against this plan, ledger commit, revise remaining batches. Batches 4+ get task-level detail at the boundary before them, per the planning conventions.
