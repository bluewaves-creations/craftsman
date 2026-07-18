# Craftsman

Craftsman is a development system for working with coding agents without
lowering the bar: the human owns the vision and the spec, the agent does
the work, and a deterministic CLI — this repo — delivers every verdict as
an exit code. No LLM ever judges whether code works here.

Three legs, one contract:

- **`craftsman` CLI** (`cli/`, Rust) — spec engine, per-stack verify
  adapters, gate orchestration with ratcheting baselines, docs pipeline,
  ledger commits, bootstrap. Single binary, no daemon, no telemetry.
- **Six skills** (`skills/`) — `craftsman-init/spec/plan/implement/fix/
  review`, embedded in the binary and installed by `craftsman setup`.
- **The committed contract** — `craftsman.toml`, `SPEC.md` (human-owned
  Gherkin), `AGENTS.md` (≤100 lines of rules), `.craftsman/baselines/`.

## Install

```sh
sh install.sh          # release binary if present, else cargo install; then setup
```

or by hand: `cargo install --path cli --locked && craftsman setup`.
`setup` places the six skills in `~/.agents/skills/` and links them for
Claude Code; Codex/Cursor/Gemini/opencode/Goose/Pi read the canonical dir
natively. It never destroys anything it cannot prove it wrote.

## The five-minute tour

```sh
git init my-app && cd my-app
craftsman init --name my-app --stack rust   # scaffold: config, AGENTS.md skeleton,
                                            # walking-skeleton SPEC.md, hook templates
craftsman doctor                            # prove the loop: red observed, then green
$EDITOR SPEC.md                             # the human writes scenarios (Gherkin)
craftsman spec lint                         # authoring + code-gen compatibility check
$EDITOR PLAN.md                             # batches of 2–4 scenarios each
craftsman plan lint                         # batch → scenario mapping stays honest
craftsman verify --batch 1                  # red → implement → green (exit code is the verdict)
git add -A && craftsman commit --type feat --scope batch-1 \
  --message "first behavior" --scenarios "My first scenario"
```

`craftsman commit` refuses while any gate is red and is the only writer
of the `Verified-by:` trailer — green gates become unforgeable history.
Existing codebase instead of a fresh one? `craftsman adopt --status`
starts the five-phase brownfield protocol (observe → ledger → baseline
gates → recover truth → steady state).

## Gates

Configured in `[gates]` (`off | baseline | strict`); `check-all` runs the
enabled set in order with a file-hash cache; `--changed` scopes to the
diff. `baseline` mode snapshots existing debt and fails only on new
findings, auto-ratcheting down as debt is paid.

| Gate | What it runs | Notes |
|---|---|---|
| `verify` | SPEC.md scenarios via the stack adapter | always strict; the spec is the test suite |
| `lint` | cargo fmt+clippy / ruff / biome / swiftlint / shellcheck | hermetically pinned tools |
| `arch` | `[arch] deny = ["A -> B"]` dependency direction | textual import graph, v1 |
| `security` | gitleaks · semgrep · osv-scanner | never narrowed by `--changed` |
| `health` | function/file length, complexity, duplication | craftsman's own deterministic metrics |
| `mutate` | cargo-mutants / mutmut / Stryker, diff-scoped | full runs need `--all --yes-slow` |
| `perf` `a11y` `visual` | lhci/k6 · playwright+axe · screenshot specs | refuse loudly when unconfigured |

Exit codes everywhere: `0` pass · `1` verification failure · `2` usage ·
`3` orchestrator error · `4` empty selection. Every command takes
`--json` (JSON on stdout, progress on stderr).

## Self-hosting

This repo eats its own cooking: `SPEC.md` at the root holds **34
scenarios** run by cucumber-rs through `craftsman verify`, every commit
goes through `craftsman commit`, and CI finishes with `check-all` on a
fresh runner. The `.craftsman/baselines/` ratchet is committed history.

## Repo map

| Path | What |
|---|---|
| `cli/` | the Rust binary (modules: spec, verify, gates, docs, ledger, bootstrap) |
| `skills/` | the six skills + the shared conventions file (byte-identical copies, test-enforced) |
| `docs/design/` | CLI surface + skill family designs (the authority chain) |
| `docs/plans/` | the batched implementation plan (Batches 0–8) |
| `docs/research/` | the 22-document research corpus |
| `decisions/` | ADRs + generated `index.md` (`craftsman adr index`) |
| `SPEC.md` | craftsman's own acceptance spec |

## Status

v0.1.0 — team-local. Distribution is a GitHub Release built by cargo-dist
(config committed, pinned 0.32.0) plus `install.sh`; `craftsman update`
refreshes skills from the binary and points at the reinstall path — real
self-update is future work. Known honest-undone: xcodebuild verify
variant, Linux Swift CI, python/ts mutation e2e through craftsman.
