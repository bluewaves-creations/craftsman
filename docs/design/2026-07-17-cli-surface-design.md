# Craftsman CLI — Command Surface & craftsman.toml Design

> The deterministic leg of the triad, made concrete: every command the six skills invoke, the committed config contract, and the settlement of the six deferred design decisions. Builds on the verification-cli, documentation-pipeline, brownfield, and skill-family designs.

---

## Ground rules (from research, restated as contract)

- Single Rust binary (clap), ~5ms startup, no daemon, no service, no network requirement except `docs sync` and tool installs.
- **Output contract**: every command supports `--json` (JSON to stdout, human progress to stderr; JSON Lines streaming where results arrive incrementally). Non-interactive by default; TTY-aware.
- **Exit codes** (uniform): `0` pass · `1` verification/gate failure · `2` usage error · `3` orchestrator error (missing tool, invalid config) · `4` empty selection (filter/batch matched nothing — never silent success).
- **Single-writer**: the CLI is the only writer of `.craftsman/` state, baselines, ledger trailers, and the docs cache. Skills judge; `craftsman` records.
- `--help` is a first-class documentation surface: exhaustive, example-rich, written for agents and humans.

## Command surface

```
craftsman
├── init                  # scaffold: AGENTS.md skeleton, craftsman.toml, .craftsman/, harness wiring
├── adopt                 # brownfield: --status | --phase 0..4 (resumable state in .craftsman/adoption.toml)
├── doctor                # prove the loop: config valid, tools present, one red→green round-trip
├── setup                 # install/refresh skills into agent homes (per-agent adapter table)
├── update                # self-update CLI + bundled skills (team channel: GitHub Releases)
│
├── spec
│   ├── status            # scenario inventory: green/red/undefined per feature, per batch
│   ├── lint              # Gherkin authoring + code-gen-compatibility check
│   └── gen               # (Swift/bash stacks) generate test files from SPEC.md
│
├── plan
│   └── lint              # validate PLAN.md batch→scenario mapping: names exist, no orphans,
│                         #   coverage report for the gap gear
│
├── verify                # THE gate: run scenarios via the stack adapter
│   │                     #   --batch N | --scenario "name" | --all (default)
│   │                     #   --impact [REF]  → run only scenarios the diff can affect
│   └── (adapters: pytest-bdd | cucumber-js | playwright-bdd | cucumber-rs
│                  | swift-testing codegen | bats codegen)
│
├── lint | arch | security | health | mutate | perf | a11y | visual
│                         # one subcommand per gate; all accept --changed; mutate is
│                         #   diff-scoped by default (full runs need --all --yes-slow)
├── check-all             # orchestrate enabled gates; file-hash cache; --changed
│
├── gate
│   ├── status            # per-gate mode + baseline counts + ratchet history
│   ├── baseline <gate>   # record/refresh baseline (brownfield Phase 2)
│   └── strict <gate>     # flip to strict once baseline hits zero
│
├── docs
│   ├── add | sync | status | search | get
│   └── (cache: .craftsman/docs/, keyed library@version from lockfiles)
│
├── extract               # write session knowledge: .craftsman/session/{index,batch-N,learnings}.md
├── commit                # structured ledger commit: validates type/scope, writes trailers,
│                         #   refuses if check-all (--changed) is red — the mechanical Verified-by
└── adr
    ├── index             # regenerate decisions/index.md (<500 tokens)
    └── stale             # cross-ref active ADRs vs git history of files they cite; report only
```

Everything else named by the skills maps onto this surface: the boundary gear is `verify` → `check-all` → `plan lint` (gap report) → `extract` → `commit`; the finish gear adds `adr stale` + `adr index`.

## Settled design decisions

**1. `--batch` semantics → PLAN.md-side scenario lists (not Gherkin tags).**
SPEC.md is static and human-owned; batching is planning, and it moves. Encoding batches as `@batch-N` tags would force the agent to edit SPEC.md at every replan — a standing violation of "only the human changes the spec." Instead, each PLAN.md batch carries a `Scenarios:` list; `craftsman verify --batch N` parses PLAN.md (via the gherkin-crate scenario inventory for validation), resolves names, and synthesizes each runner's native filter (marker expression, `--name` regex, `--filter`, `-f`). `plan lint` keeps the mapping honest. Gherkin tags remain available for orthogonal, durable suites (`@slow`, `@ios-only`) — human-owned like the rest of the spec.

**2. Baseline format → hybrid: native where the tool has one, craftsman snapshot where it doesn't.**
Wrap native baselines for SwiftLint (`--write-baseline`), ESLint (bulk suppressions + prune), and Semgrep (`SEMGREP_BASELINE_REF`) — they are battle-tested and diff-aware. For everything without one (Ruff, health, mutate, perf/a11y/visual budgets), `craftsman gate baseline` writes a unified Betterer-style snapshot in `.craftsman/baselines/<gate>.json` (counts keyed by rule+file). Both kinds present one UX (`gate status`, auto-ratchet on improvement, scheduled pruning) and one ledger trail.

**3. Skills bundling → embedded in the binary, installed by `craftsman setup`.**
The repo's `skills/` directory is embedded at build time (`include_dir!`), so one artifact carries CLI + skills and `craftsman update` refreshes both — the hatch-wheel pattern translated to cargo. `setup` ports Fusion's agent table verbatim: canonical home `~/.agents/skills/` (+ project `.agents/skills/`), symlink adapter for Claude Code, copy for Windsurf, standard mode for Codex/Cursor/Gemini/opencode/Goose, attribution-checked never-destroy semantics. Team-only: distribution is a GitHub Release + a <100-line `install.sh`; no Homebrew/npm/marketplace until there's a reason.

**4. `docs search` → embedded grep, no index.**
Use the `grep`/`ignore` crates in-process (the ripgrep internals) over the version-pinned markdown cache — zero external dependency, no FTS index to maintain. Revisit only if the cache outgrows grep ergonomics.

**5. Lint/security internals → direct declarative adapters, not a qlty/trunk wrap.**
The competitive doc leaned "wrap qlty"; the deciding facts for a team-only tool: our stack list is closed (≈12 tools: ruff, pyright, biome/eslint, tsc, clippy, swift-format/SwiftLint, shellcheck/shfmt, semgrep, gitleaks, osv-scanner, dependency-cruiser/import-linter), qlty is pre-1.0 under BUSL, and wrapping adds a second meta-layer between craftsman and the tools it must pin hermetically. So: trunk-*style* declarative adapters (tool, version pin, invocation, output parser, success codes) as data files in the binary, tools installed hermetically into `~/.craftsman/tools/<tool>@<version>/`. Revisit at 1.0 if adapter maintenance exceeds ~a day per quarter.

**6. Enforcement without hooks (Codex et al.) → the commit gate + CI backstop.**
Where the harness has hooks (Claude Code, Cursor), wire `craftsman check-all --changed` into stop/pre-commit events at `init`. Where it doesn't (Codex), enforcement is structural instead of event-driven: `craftsman commit` **refuses** to create a ledger commit while gates are red (the trailer is written by the CLI only when the gates actually passed — `Verified-by:` becomes unforgeable), and CI runs `craftsman check-all` as the backstop. Convention can be ignored; the commit gate cannot.

## craftsman.toml

```toml
# craftsman.toml — committed; the contract between human, agent, CLI, and CI
[project]
name = "acme-app"
stacks = ["swift-apple"]            # swift-apple | swift | python | typescript | rust | bash
spec = "SPEC.md"
plan = "PLAN.md"
cli-version = "0.4"                 # pinned; launcher warns on drift (trunk lesson)

[verify]
runner = "swift-testing-codegen"    # per-stack default, overridable
scheme = "AcmeApp"                  # xcodebuild stacks only
destination = "platform=iOS Simulator,name=iPhone 17"

[gates]                             # absent gate = off
verify   = "strict"                 # verify is always strict — baselines never apply to the spec
lint     = "baseline"
arch     = "strict"
security = "baseline"
health   = "baseline"
mutate   = "strict"                 # diff-scoped by design
a11y     = "strict"
visual   = "off"

[gates.tools]                       # version pins; adapters install hermetically
swiftlint = "0.57.0"
semgrep   = "1.130.0"
gitleaks  = "8.24.0"

[gates.arch.rules]                  # fitness functions (per-stack rule engine)
deny = ["Domain -> Infra", "UI -> Persistence"]
max-file-lines = 400

[budgets]
perf.p95_ms = 200                   # k6 / lighthouse budgets, stack-dependent
tokens.agents-md-lines = 100        # init enforces the AGENTS.md budget mechanically

[docs]
cache = ".craftsman/docs"           # sources live in AGENTS.md (Documentation Sources table)
```

`.craftsman/` layout (gitignored except `baselines/` and `adoption.toml`, which are committed):

```
.craftsman/
├── baselines/<gate>.json    # committed — the ratchet's memory
├── adoption.toml            # committed — brownfield phase state
├── session/                 # gitignored — extraction target, survives compaction
├── docs/                    # gitignored — version-pinned doc cache
└── cache/                   # gitignored — file-hash gate cache
```

## `verify --impact` (the TDAD mechanism) — design

Coverage-mapped impact, built from data the CLI already produces:

1. During every `verify` run, adapters record per-scenario file coverage where the stack supports it cheaply (pytest-cov contexts; c8/istanbul per-test; cargo llvm-cov; xcresult coverage; bats: file-level only) into `.craftsman/cache/impact-map.json`.
2. `verify --impact [REF]` diffs the working tree against REF (default: last ledger commit), intersects changed files with the map, and runs the union of: scenarios whose covered files changed + scenarios whose glue/generated test files changed + scenarios with no map entry yet (unknown = affected).
3. Cold start or stale map → falls back to `--all` loudly (never silently narrower).

This is deliberately conservative: false positives cost seconds; false negatives cost regressions. **Spike required** (flagged in the research README): per-scenario coverage overhead on the Swift/xcresult path.

## What the CLI deliberately does not do

- No LLM calls, ever. No network in the verdict path.
- No session memory, plan-mode UX, or orchestration — harness territory.
- No linter/scanner reimplementation — adapters orchestrate pinned tools.
- No SaaS, telemetry, or accounts.

## Open items

1. **Spikes** (before implementation): Swift Testing code-gen round-trip (`spec gen` → `swift test --filter` → JSONL parse); per-scenario coverage cost for the impact map; `swift test` event-stream behavior on Linux.
2. **Adapter data-file schema** — the declarative format for gate adapters (command, parser, success codes, baseline kind); design at implementation time against the first three tools.
3. **`arch` rule engine for Swift/Rust** — no incumbent exists (verified); rule syntax above is a placeholder pending its own short design note.
4. **Hook wiring templates** per harness (`init` writes them) — enumerate exact events for Claude Code and Cursor at authoring time.
