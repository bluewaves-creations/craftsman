# Brownfield Adoption (adopt)

Loaded to bring an existing codebase under Craftsman. The ordering is the whole game: **ledger before gates, gates before specs, specs before change.** Phase state lives in `.craftsman/adoption.toml` (CLI-written, committed) — `craftsman adopt --status` tells you where a previous session stopped; resume there, never restart.

## Phase 0 — Observe (read-only)

No code changes, no config changes. Map the repo: entry points, hotspots (churn × complexity from `git log`), test coverage reality, and the build/test/run commands — **executed, not inferred**. Output: a research doc at `docs/craftsman/adoption-survey.md` with every claim labeled `verified` (you ran it / a test proves it), `inferred` (your reading, unexecuted), or `gap` (unknown). Human reads it before Phase 1.

## Phase 1 — Ledger (process only)

- AGENTS.md from Phase 0's **verified** material + human attestations only. Inferred architecture stays in the survey doc, clearly labeled — never in AGENTS.md.
- `craftsman.toml` with all gates `off`.
- Commit trailers start now: every commit from here on goes through `craftsman commit`.
- `decisions/ADR-000`: state of the system at adoption — the baseline decision record.

Zero code risk; the ledger starts accumulating archaeology immediately.

## Phase 2 — Hold the line (baseline gates)

Per gate: `craftsman gate baseline <gate>` — wraps the tool's native baseline where one exists, writes the unified snapshot where not. Then flip the gate to `baseline` mode in craftsman.toml. Baselines are committed. From this commit forward the codebase cannot get worse: CI and `craftsman commit` fail on *new* violations only; improvements ratchet automatically. Confirm each gate's baseline count with the human — the numbers are the debt made visible.

`verify` stays effectively empty (no scenarios yet) — that's expected; it is strict from birth.

## Phase 3 — Recover truth (scoped)

Only for hotspots and critical paths the human names — never whole-codebase backfill. Route to craftsman-spec `recover` per area: characterization tests at seams → human approves snapshots → `verified` scenarios enter SPEC.md's "Current behavior (recovered)" section. Each area lands as a `retro-spec` ledger commit. Bugs discovered while pinning are filed, not silently fixed (fixing routes to craftsman-fix, after this area's pinning is committed).

## Phase 4 — Steady state

- **New work**: full Craftsman, strict, via sprout — new modules are strict-mode islands with their own scenarios.
- **Legacy on touch**: changing an unpinned area → recover that area first (delta-scoped), then change it.
- **Graduation**: when a module's baseline hits zero for a gate, `craftsman gate strict <gate>` for it — permanently. Report graduations; they are the adoption's progress metric.

## Never

- Never skip a phase or run one before its predecessor's commit exists.
- Never turn a gate strict while its baseline count is nonzero.
- Never let an `inferred` claim into AGENTS.md or SPEC.md.
- Never fix bugs during Phase 0–2 — observe, record, hold the line; fixing starts with the ledger in place.
