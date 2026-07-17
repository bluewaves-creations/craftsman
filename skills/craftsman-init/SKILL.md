---
name: craftsman-init
description: >
  Craftsman bootstrap — brings a repo under the methodology. Gears: new
  (greenfield: minimal AGENTS.md interview, craftsman.toml, verify harness, one
  proven red→green scenario), adopt (brownfield: observe → ledger → baseline
  gates → recover truth → steady state), upgrade (refresh conventions + CLI
  pin). Use for "set up craftsman", "init craftsman", "adopt craftsman in this
  repo", "bring this codebase under craftsman". All gears write files — always
  confirm scope before writing. For drafting scenarios afterwards use
  craftsman-spec. Requires the craftsman CLI on PATH; if missing, point to the
  installer and stop.
license: MIT
compatibility: Requires the craftsman CLI on PATH and git.
---

# Craftsman Init

You bring a repository under the Craftsman methodology — greenfield or brownfield — and you prove the loop closes before calling it done. Read `references/craftsman-conventions.md` once per session before any gear runs.

Every gear in this skill is destructive: it writes files. There is **no default gear** — if the request doesn't name one clearly, ask. Before writing anything, state exactly which files will be created or modified and get confirmation.

## Routing

| Signal | Gear | Load |
|---|---|---|
| Empty or new repo; "set up craftsman", "init" | `new` | `references/new.md` |
| Existing codebase; "adopt", "bring under craftsman" | `adopt` | `references/adopt.md` |
| "upgrade craftsman", conventions drift reported | `upgrade` | `references/upgrade.md` |

If the repo already has a `craftsman.toml`, `new` is wrong — confirm whether the human wants `adopt` (resume) or `upgrade`.

## Shared rules

- **AGENTS.md is observed, not inferred.** Every line is either a command you executed successfully or a fact the human attested in the interview. Inferred architecture goes into a separate, labeled research doc — never into AGENTS.md. Enforce the length budget (`budgets.tokens.agents-md-lines` in craftsman.toml, default 100).
- **The loop must close.** No gear finishes until `craftsman doctor` passes: config valid, tools present, and one trivial scenario proven red → green through `craftsman verify`.
- **Apple projects**: probe `xcrun agent skills export` — if it succeeds, install Apple's skills alongside the Craftsman family and record the delegation in AGENTS.md ("SwiftUI/testing idiom: defer to Apple's skills").
- **Harness wiring**: where the harness supports hooks (Claude Code, Cursor), write the gate-enforcement hook templates. Where it doesn't, note in AGENTS.md that `craftsman commit` and CI are the enforcement points.
- Not a git repo → offer `git init` first; the ledger needs git.

## Never

- Never write AGENTS.md content the human didn't attest or you didn't observe by running a command.
- Never scaffold over existing files without listing them first.
- Never mark bootstrap complete without a passing `craftsman doctor`.
- Never skip baseline recording when adopting — a gate turned on strict against legacy code is a gate everyone learns to ignore.
