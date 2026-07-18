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

- [x] `init` (non-interactive scaffold: craftsman.toml verify+lint strict / security baseline, AGENTS.md skeleton with budget marker, walking-skeleton SPEC.md, .craftsman/ dirs, merged .gitignore, CLAUDE.md symlink with pointer-file fallback, hook templates), `adopt` (five-phase state machine in .craftsman/adoption.toml, sequencing enforced, transitions record timestamp+HEAD; phase 1 writes gates-off config + ADR-000, phase 2 records baselines, 0/3/4 state-only), `setup` (six skills embedded via include_dir, canonical ~/.agents/skills + Fusion agent table; attribution-checked never-destroy via .craftsman-setup sha256 sentinels; --remove mirrors, --status reports; conventions byte-identity test-enforced), `update` (honest team-local: version + skill refresh + reinstall pointer).
- [x] cargo-dist config (real `dist init`, cargo-dist 0.32.0 pinned, dist-workspace.toml, three targets) + install.sh (67 lines, POSIX) + `--version` git sha via build.rs; CI gains an uncached `check-all` step with ~/.craftsman/tools cached on the pins.
- [x] Contract sweep (cli/tests/contract.rs): --help everywhere, exit-code docs on verdict commands, bad flag → 2, missing config → 3, offline happy paths parse as JSON. Audit fixes: --json added to gate baseline/strict + docs get; exit-code docs added to security/doctor/gate baseline; stdout/stderr mixing fixed in gate status, doctor, docs status, docs search. The known 6b nit reconciled: health said "42 baselined" vs gate status 41 — 41 was true (two identical findings collide into one fingerprint; baselined now counts distinct fingerprints, one unit both places).
- [x] Finish: full `cargo test` + uncached `check-all` green, README (~120 lines, install + tour + gate table), adr index regenerated, tagged v0.1.0 (annotated, not pushed).
- Notes: the Claude Code hooks JSON shape was verified against working settings.json files on this machine (top-level `hooks` → event → matcher groups → command hooks); Cursor's could not be verified from an offline docs source, so init ships an inert `.cursor/craftsman-hooks.json.template` with a note — never an invented schema. `dist generate` (release.yml) is deferred until the repo has a GitHub remote (it requires a repository URL); the config is committed and pinned. `dist init --yes` ignored the `-t` flags — targets trimmed by hand. Setup is stricter than Fusion's original: canonical trees are replaced only with sentinel/digest proof, so even a user-modified canonical copy survives without --force.

Scenarios:
- Init scaffolds a project that doctor accepts
- Init refuses to overwrite without force
- Setup installs skills with attribution sentinels
- Adopt enforces phase ordering

---

Boundary rule: after each batch — full test suite, clippy `-D warnings`, gap check against this plan, ledger commit, revise remaining batches. Batches 4+ get task-level detail at the boundary before them, per the planning conventions.

---

## Batch 9 — 100% production grade (gap closure before dogfood)

*(planned 2026-07-18 from the v0.1.0 audit: every honest-undone item across batches 2–8 plus the four audit findings. Definition of "100%", mechanical: (a) the honest-undone register is empty — each item either implemented and proven, or descoped by an ADR the human approved; (b) all baselines at zero and their gates strict, or scope-excluded by committed config with recorded rationale; (c) every gate live-proven at least once; (d) CI green on a real remote; (e) contract sweep covers every command incl. network paths. Split 9a/9b/9c, sequential boundaries.)*

### Batch 9a — Apple completion (the user's flagship stack)

- [x] **xcodebuild adapter** (`verify/adapters/xcodebuild.rs`): `[verify.swift] scheme`+`destination` drives `xcodebuild test -scheme … -destination … -resultBundlePath <tmp>.xcresult` (+ `-only-testing:` from ADR-001 identity for --scenario/--batch); parse `xcrun xcresulttool get test-results tests` JSON (Xcode 16+ subcommand, undocumented-but-stable schema — build the parser from a REAL captured xcresult of the S1 spike package opened as an Xcode project, committed as fixture); exit 65 is ambiguous — always parse the bundle, never trust the code; map stub-marker failures → Undefined (same dialect as SwiftPM). Live round-trip proof on this machine (Xcode 27 present): scaffold an app-shaped fixture, observe pass/undefined/fail through craftsman. *(Done — this machine has Xcode 26.6, not 27. Probed reality: xcodebuild tests the SwiftPM package's synthesized scheme directly (no .xcodeproj; product schemes, or `<name>-Package` when no products) — that IS the fixture (`cli/tests/fixtures/xcode-app/`). `-only-testing` needs the exact `` Target/Suite/`name`(signature) `` identifier — `()`, `(a:b:)`, `(_:)` all probed; wrong signature silently matches 0. Even a scheme-not-found error writes a test-less bundle, so an empty parse from a failing xcodebuild is a tool failure. Scrubbed real-bundle JSON committed as `cli/tests/fixtures/xcresult-tests.json`; live round trip pass/undefined/fail + `--scenario` selection proven; #[ignore]-gated integration test in `cli/tests/xcodebuild.rs` (~42s cold).)*
- [x] **Apple a11y path**: `[a11y] scheme`/`ui-test-target` variant — the gate runs `xcodebuild test -only-testing:<UI target>` where user-land XCUITests call `performAccessibilityAudit()`; findings from the xcresult. Template UI-test file emitted by `spec gen --a11y-stub` (write-once). Live-proven against the fixture app once. *(Done with one honest limit: gate plumbing live-proven against the fixture package's test target — 2 findings extracted from a real bundle — but the audit itself cannot run there: XCUITest needs an app host and SwiftPM packages cannot declare one (observed: "Tests in the target … can't be run because … isn't a member of the specified test plan or scheme"). A real .xcodeproj app target is the user-land prerequisite; finding extraction is proven on the identical bundle format.)*
- [x] **SwiftLint native baseline**: `gate baseline lint` writes `--write-baseline` when a swift stack is configured; proven against a seeded-violation swift fixture (closes the 6a honest-undone). *(Done — red→record→green→red round trip live with hermetic swiftlint 0.57.0. Learned: SwiftLint's own baseline matching absorbs additional violations in files that already carry baselined ones; new files surface.)*
- [x] **Swift/TS/Rust impact narrowing**: per-scenario file maps from what each runner already tells us — ts: pickle URIs + step-definition files from NDJSON; swift: generated-glue file + Steps.swift; rust: harness target file. Narrowing rule stays conservative (unmapped = run; any glue change = run all) but a docs-only or unrelated-stack diff now genuinely narrows. Behavior per stack documented + unit-tested (closes the 4/8 honest-undones). *(Done — impact map v2: glue maps carry the stack `tree`; docs-only diff on this repo proven live: `verify --impact HEAD` → 0-of-34 with the loud note, exit 0; in-tree counter-proof runs all 34.)*
- Success: an xcodeproj app fixture goes red→green through `craftsman verify` on this machine; a11y gate live-proven once; `gate baseline lint` writes a real SwiftLint baseline; impact narrows in a proven case per stack.

### Batch 9b — Verification completeness + contract polish

- [x] **py/ts mutate e2e through craftsman**: fixture-project runs asserting score parsing + threshold verdicts (mutmut 2.5.1 aggregate limitation stays documented; Stryker gets a committed minimal config in the ts fixture). *(Done — cli/tests/mutate.rs drives `craftsman mutate --json` in disposable git projects built from committed fixture pieces (python-todo/mutation/, ts-todo src/ + stryker.config.json with the bun-test command runner), seeded diffs, min-score 100 for a deterministic survivor-red. Measured 2.1s/1.1s warm — unignored. The e2e exposed a real parser bug: mutmut's `--no-progress` suppresses the counts line entirely; the adapter now parses the spinner's final CR-delimited segment (real capture is a unit fixture).)*
- [x] **Runtime gates live-proven once**: a tiny committed static-site fixture (plain html + one playwright spec + one axe spec + lhci config); `visual`, `a11y` (web path), `perf` each run live locally behind a browser-available check that skips loudly (closes "schema-doc-constructed samples" and gives the parsers real artifacts as fixtures). *(Done — cli/tests/fixtures/static-site/ (accessible index + seeded-issue variant + committed darwin baseline screenshot + generous/absurd lighthouse budgets, bun.lock committed); cli/tests/runtime_gates.rs proves green AND red for all three through the binary (42.5s wall, unignored). Decisions: playwright loads file:// URLs (no server); lhci serves via staticDistDir; CHROME_PATH points at the Playwright chromium (no system Chrome here). REAL captured lhci + playwright artifacts committed under fixtures/runtime/ and wired into the parser unit tests; k6 stays the one constructed sample (no live k6 run — noted inline).)*
- [x] **biome line numbers**: re-derive line/col from byte spans (read the file, count); finding parity test. *(Done — better than planned: biome 2.2.5's diagnostics embed the file text in `location.sourceCode`, so the parser derives lines from the byte span with no file read; parity test against a real captured report, fixtures/biome-report.json.)*
- [x] **`spec status` reads recorded results**: verify runs persist normalized results to `.craftsman/cache/last-verify.json`; `spec status` shows green/red/unknown per scenario + per-batch rollup (closes the Batch 2 deferral). *(Done — record carries version/timestamp/HEAD/per-stack results, single-writer, overwritten per run (filtered runs record partially; absent = unknown — merging across runs would mix HEADs); staleness note when HEAD moved; per-batch rollup from the plan's Scenarios lists; SPEC scenario added.)*
- [x] **Contract sweep completion**: `--json` happy-path coverage for security/mutate/docs-sync via fixtures (offline security run against pre-resolved tools; mutate on the tiny rust fixture; docs sync against a local file source); every command now swept. *(Done — security/mutate skip LOUDLY when ~/.craftsman/tools lacks the pins (the sweep never downloads); mutate's offline path is the clean-tree "nothing to mutate" pass, score paths live in tests/mutate.rs; test-header inventory updated.)*
- [x] **docs sources docc/objects-inv/dts implemented**: docc = `swift package generate-documentation --enable-experimental-markdown-output` into the cache (probe support; if the toolchain flag is absent, record observed reality and keep refusal WITH the probe result); objects-inv = parse the zlib inventory (name→url index; search over the index, pages fetched per-page-md on demand); dts = harvest `node_modules/<pkg>/**/*.d.ts` into the cache verbatim. Each with a real dogfood target (swift-nio or the spike package for docc; a pydantic objects.inv; zod's dts from the s2 sample). *(Done — probed reality: `swift package generate-documentation` is a PACKAGE PLUGIN (absent without a swift-docc-plugin dependency, both toolchains), so docc sync probes the plugin and falls back to `swift build -emit-symbol-graph` (private scratch) + `xcrun docc convert --enable-experimental-markdown-output` (flag present on Xcode 26.6 AND 27.0 — probed; absent flag = refusal citing the probe). objects-inv: flate2-inflated v2 inventory → searchable name→URL index page + inventory.json; `docs get` fetches a target page on demand — the ONE documented network exception outside sync (chosen over prefetching unbounded HTML). dts: verbatim harvest keyed by the installed package.json version, nested node_modules excluded, max-pages bounded. Dogfooded live: specspike 23 docc pages, pydantic 1588-object inventory, zod@4.3.5 200 declaration files.)*
- Success: contract sweep green over the full command surface; all six docs source types sync something real; runtime gates each have one live artifact in fixtures. **Met 2026-07-18.**

Scenarios:
- Spec status shows the last verify verdicts

### Batch 9c — Debt zero + infrastructure

- [x] **Gate scope config** (`[gates] exclude = ["spikes/**"]`): spikes are frozen evidence, not shipped code — committed exclusion with rationale comment; security re-baseline → expected 0 (the 2 RUSTSEC hits live in a spike sample lockfile); ALSO `cargo update -p quick-xml` in the spike sample so the advisory is actually gone, belt and braces (lockfile refresh ≠ evidence tampering; note in commit body). *(Done — exclusion implemented once (gates::scope, applied centrally in the shared epilogue + the file-census points); security re-recorded at 0 and flipped strict. The belt-and-braces bump is NOT available: the fix is quick-xml 0.41.0 only, junit-report 0.9.0 (latest under cucumber 0.23) requires ^0.39, `cargo update` locks 0 packages — exclusion is the whole resolution, advisory stays visible in the frozen spike lockfile.)*
- [x] **Health burn-down to strict**: refactor the real offenders in cli/src (health.rs 8 findings, normalize.rs 5, remainder list from `gate status --json`) in small `refactor:` commits (fix/refactor separation holds; verify green after each); spikes excluded by scope. Target: `gate strict health` flips (baseline 0). If any single finding is genuinely correct-as-is, a scoped allow with reason comment counts as resolution — no naked suppressions. *(Done — 41 baselined → 0 across eight refactor commits: normalize/health/main/verify/gates/codegen/config splits by responsibility plus genuine extractions; the harness became tests/spec/{main,project_steps,repo_steps}.rs (cucumber step registration is link-time — 35/35 green). Inline `craftsman-health: allow <rule> — <reason>` support added first (reasons mandatory, invalid directives are findings); exactly 2 allows ship: branch_words max-complexity (keyword table is data) and the swift round-trip max-function-lines (live narrative). `gate strict health` flipped; all five enabled gates strict at 0.)*
- [ ] **GitHub remote + CI first run** *(human-gated: org/name/visibility — ask, do not invent)*: create remote, push, confirm CI green on both runners (first REAL CI execution); `dist generate` release workflow committed once the repository URL exists. *(REMOTE-GATED — untouched in 9c pending the human's repo decision; ADR-005 §1–2.)*
- [x] **swift-linux CI**: verify swift-actions/setup-swift (or swiftly) Swift 6.2+ on ubuntu-24.04 by running it (a throwaway workflow on the new remote is the honest probe); enable the parked job or re-park with the observed failure. *(Desk-verified from official sources: swift.org ships 6.2–6.3.3 for ubuntu-24.04; setup-swift v2.4.0 lists 24.04 + 6.2/6.2.1 in its version tables. Parked job re-pinned to v2.4.0 but LEFT COMMENTED — open issue #677 (GPG on 24.04) was never closed, so the first remote run is the canary; fallback vapor/swiftly-action@v0.2.1. Live probe remote-gated; ADR-005 §4.)*
- [x] **Cursor hooks template**: verify the current hooks schema against Cursor's docs (network available); activate the template or keep inert citing the verified shape/absence. *(Verified at cursor.com/docs/agent/hooks (hooks beta since 1.7, agent-loop events since 3.11): project .cursor/hooks.json, top-level version+hooks, event → [{command, timeout}], exit 2 blocks. init now writes a LIVE .cursor/hooks.json whose stop hook blocks on red `check-all --changed`; the inert template is gone.)*
- [ ] **`craftsman update` real path**: axoupdater against the GitHub Releases once they exist; falls back to the current guidance when no release channel — implement behind release availability (depends on the remote task). *(REMOTE-GATED — needs Releases to exist; ADR-005 §3.)*
- [x] **ADR-005 — descope register**: anything above that lands as "won't do for 0.2" (candidates: none expected; xcodebuild-on-Linux is N/A by nature) gets its ADR line, human-approved; the honest-undone register in this plan is then EMPTY. *(Written (Proposed — the human approves/amends): the four remote-gated items, k6 live artifact, live performAccessibilityAudit, sub-lettered rollup, pydantic 0.0.0 inventory stamp, and the recorded mutate-at-boundaries policy. This plan's honest-undone register now POINTS THERE.)*
- Success: `gate status` shows security+health strict at 0; CI green on the remote; the register empty or ADR'd; tag v0.2.0. *(Met locally 2026-07-18 except the remote-gated third: security+health strict at 0 (all five enabled gates strict, baselines empty); register ADR'd (ADR-005, Proposed). CI-on-remote pending the human's repo decision — tagged v0.2.0-rc1, not v0.2.0: 0.2.0 stays reserved for CI-green on a real remote.)*


## Batch 10 — Release channel (v0.2.0)

*(revised in at the 9c boundary; delta APPROVED by the human 2026-07-18 and committed as SPEC.delta.md — bcb7dc2. Its four scenarios — "Update without an install receipt explains the reinstall path", "Update refreshes the installed skills from the binary", "Update with an unreachable release channel fails loudly", "Update self-updates to the latest release" (@requires-network) — merge into SPEC.md at this boundary; the Scenarios list below stays empty until then so plan lint tracks only executed truth. Dogfood learning folded in: every dogfood run so far used cli/target/debug directly — the install path (install.sh → release binary → craftsman setup) has never been exercised on this machine; this batch ends by exercising it for real.)*

Scenarios: (in SPEC.delta.md until the boundary merge — four update scenarios, approved)

Tasks:
- Implement axoupdater-backed `craftsman update`: install-receipt detection (NoReceipt → exit 0 + current version + install.sh pointer), skill refresh from the embedded copies, unreachable-channel failure naming the channel (docs: axoupdater 0.10.0 via craftsman docs — already declared in AGENTS.md)
- Wire the three hermetic delta scenarios into the harness red-first; implement to green
- `dist generate` — commit the release workflow now that the repository URL exists
- Enable the swift-linux canary job (setup-swift v2.4.0, desk-verified in 9c) and observe its first live run; re-park with the observed failure if issue #677 bites
- ADR-005: human approves/amends the deferral register
- Cut the first GitHub Release; tag v0.2.0 at CI-green; merge SPEC.delta.md into SPEC.md and delete the delta file
- Redeploy for real: `sh install.sh` against the v0.2.0 release, `craftsman setup`, `craftsman doctor` green from the installed binary — retire the debug-build dogfood path; run the @requires-network self-update scenario against the live release

Success: craftsman verify exits 0 with the merged update scenarios AND CI green on the release workflow's first run AND doctor green from the installed (not debug) binary

## Batch 11 — Retro-spec catch-up (whole-surface, human-directed)

*(directed by the human 2026-07-18: the spec catches up with the complete implemented behavior. Justified exception to recover's no-backfill rule, recorded here: this spec is executable and continuously verified — recovered scenarios cannot rot — and every command is an active critical path. Recover rules still bind: verified-only, each scenario citing its characterization test; unpinned behavior becomes labeled gap work items, never scenarios.)*

*(status 2026-07-18: the recover draft is COMPLETE — SPEC.recover.md, 604 lines: ~85 behaviors inventoried, 23 already covered by the existing 35 scenarios, 52 proposed scenarios each citing a passing test or one of 13 executed CLI observations, 9 unpinned behaviors registered as GAP-R01..R09 and routed to Batch 12. AWAITING HUMAN APPROVAL — wiring does not start before it.)*

Scenarios: (in SPEC.recover.md until approval + boundary merge — 52 recovered scenarios, drafted)

Tasks:
- Human reviews SPEC.recover.md: approve whole, amend, or trim — cuts are cheap now, expensive after wiring
- Wire approved scenarios into the cucumber-rs harness in citation order (densest areas first: docs 7, verify+impact 7, gate modes 5, adopt 4, setup 4); merge into SPEC.md under "Current behavior (recovered)" green, respecting the Batch 10 delta ordering
- Batch the wiring in verify-green sub-steps of ≤10 scenarios so a red harness never blocks the commit gate mid-merge
- Delete SPEC.recover.md once merged; its gap register survives as Batch 12

Success: craftsman verify exits 0 with all merged recovered scenarios; spec lint clean; plan lint clean

## Batch 12 — Gap closure (GAP-R01..R09, test-first)

*(scaffolded from the recover inventory 2026-07-18: nine behaviors the retro-spec could not cite a test for. Recover rules bind — no scenario lands until a characterization test pins the behavior. Order: cheapest pins first; each gap becomes one `test:` commit (test only, proving current behavior) and, where the behavior deserves a spec promise, a one-line spec delta the human approves at the boundary.)*

Scenarios: (none yet — scenarios are drafted only from tests this batch writes; human approves the resulting delta)

Tasks:
- GAP-R01 — drive `adopt --start-phase 2` end-to-end in cli/tests/bootstrap.rs: baseline recorded for every baseline-mode gate (adopt.rs:312 never executed by a test)
- GAP-R02 — CLI-level test: `verify --impact` computed-empty set exits 0 with the loud "nothing to run" note, distinct from exit 4
- GAP-R03 — orchestration tests: `check-all --changed` maps verify to impact selection and narrows lint; `verify --batch N` warns on plan drift and runs the found subset
- GAP-R04 — `lint --changed` narrows to files changed against HEAD
- GAP-R05 — security threshold partition: below-threshold findings inform, never block
- GAP-R06 — broken scanner is exit 3, never a green security gate
- GAP-R07 — `adr stale` staleness verdict from git history (flow test, not just cited-path extraction)
- GAP-R08 — `docs get` objects-inv on-demand fetch-then-cache path (hermetic: local file:// inventory)
- GAP-R09 — `Dependency:` trailer rendering with a non-empty dependency list
- craftsman-spec delta for the subset worth promising in SPEC.md — human approves; merge at boundary

Success: craftsman verify exits 0; cargo test green with all nine pins; any approved gap scenarios merged green
