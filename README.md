# Craftsman

[![CI](https://github.com/bluewaves-creations/craftsman/actions/workflows/ci.yml/badge.svg)](https://github.com/bluewaves-creations/craftsman/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/bluewaves-creations/craftsman)](https://github.com/bluewaves-creations/craftsman/releases/latest)
[![License: MIT](https://img.shields.io/github/license/bluewaves-creations/craftsman)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024_edition-orange.svg)](cli/Cargo.toml)

**The human owns the vision. The agent does the work. The machine says pass.**

Craftsman is a development system for working with coding agents without
lowering the bar: the human owns the vision and the spec, the agent does
the work, and a deterministic CLI — this repo — delivers every verdict as
an exit code. No LLM ever judges whether code works here. Not once,
anywhere in the binary.

Why that matters, in one paragraph: LLM-as-judge recognizes correct code
52–78% of the time; mechanical, impact-mapped test feedback cut agent
regressions by 70% where "please do TDD" prompts made them worse. So
Craftsman moves every quality rule that *can* be mechanized into a gate,
and keeps prose only for what machines can't hold — taste and vision.
The long-form case is in [the paper](docs/2026-07-18-craftsman-paper.md).

Three legs, one contract:

- **`craftsman` CLI** (`cli/`, Rust) — spec engine, per-stack verify
  adapters, gate orchestration with ratcheting baselines, offline docs
  pipeline, ledger commits, receipt-driven self-update, bootstrap.
  Single binary, no daemon, no telemetry, no network in any verdict path.
- **Six skills** (`skills/`) — `craftsman-init/spec/plan/implement/fix/
  review`, embedded in the binary and installed by `craftsman setup`.
  Agent-agnostic per the [agentskills](https://agentskills.io) spec.
- **The committed contract** — `craftsman.toml`, `SPEC.md` (human-owned
  Gherkin), `AGENTS.md` (≤100 lines of rules), `.craftsman/baselines/`.

## Install

From the latest GitHub Release (recommended — writes the install receipt
that powers `craftsman update` self-updates):

```sh
curl -LsSf https://github.com/bluewaves-creations/craftsman/releases/latest/download/craftsman-installer.sh | sh
craftsman setup     # installs the six skills for every agent on the machine
```

Alternatives: `sh install.sh` from a checkout (offline; release binary if
present, else cargo), or `cargo install --path cli --locked`.

`setup` places the skills in `~/.agents/skills/` and links them for
Claude Code; Codex/Cursor/Gemini/opencode/Goose/Pi read the canonical dir
natively. It never destroys anything it cannot prove it wrote, and
`craftsman update` keeps both binary and skills current from then on.

> **Replaces Superpowers-style skill packs.** Many agent harnesses ship
> with or recommend broad workflow plugins (Superpowers and its
> descendants). Craftsman covers that ground with a different contract —
> machine verdicts instead of self-assessment — and the two will compete
> for the same triggers ("write tests", "plan this", "fix the bug").
> Uninstall overlapping packs before `craftsman setup`, or expect your
> agent to route work to two methodologies at once.

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

## Stacks

Swift (SwiftPM + Swift Testing, and `xcodebuild` for Apple app targets),
Python (uv + pytest-bdd), TypeScript (bun + cucumber-js), Rust
(cucumber-rs), Bash (bats) — one Gherkin dialect, one result schema, one
exit-code contract across all of them. On Apple platforms Craftsman
composes with Xcode's exportable Agent Skills rather than duplicating
them: Apple's skills own platform idiom, Craftsman owns process.

## Self-hosting

This repo eats its own cooking: `SPEC.md` at the root holds **44
scenarios** (43 hermetic, one `@requires-network`, run live against the
real release channel) executed by cucumber-rs through `craftsman verify`;
every commit goes through `craftsman commit`; CI finishes with
`check-all` on fresh macOS and Linux runners plus a Swift-on-Linux
canary. All five enabled gates run strict at zero baselines. The paper
you can hand to a colleague lives at
[`docs/2026-07-18-craftsman-paper.md`](docs/2026-07-18-craftsman-paper.md).

## Repo map

| Path | What |
|---|---|
| `cli/` | the Rust binary (modules: spec, verify, gates, docs, ledger, bootstrap) |
| `skills/` | the six skills + the shared conventions file (byte-identical copies, test-enforced) |
| `docs/design/` | CLI surface + skill family designs (the authority chain) |
| `docs/plans/` | the batched implementation plan + the dogfood program |
| `docs/research/` | the 24-document research corpus (claims graded by strength) |
| `decisions/` | ADRs + generated `index.md` (`craftsman adr index`) |
| `SPEC.md` | craftsman's own acceptance spec |

## Status

**v0.3.0** — released via cargo-dist (shell installer + macOS arm64/x86_64
and Linux x86_64 tarballs). `craftsman update` self-updates from the
release channel behind the install receipt and refreshes the installed
skills from the binary — proven live by the v0.2.0 → v0.3.0 update itself.
This release hardens the verdict path from the first external dogfood:
verify never installs dependencies (a `bunx` auto-fetch once executed a
registry dependency-confusion stub — now structurally impossible),
`craftsman commit` can make a repository's first commit, the typescript
scaffold produces a runner-discoverable spec, and `doctor` audits the
pinned gate tools. Remaining honest-undone lives in the plan's gap
register (Batch 12) and [ADR-005](decisions/) — this project keeps a
public list of what is *not* finished, because a system built on
unforgeable verdicts doesn't get to round up about itself.

## License

[MIT](LICENSE) © 2026 Bluewaves
