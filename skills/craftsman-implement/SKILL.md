---
name: craftsman-implement
description: >
  Craftsman execution — turns red scenarios green at production grade. Use
  for "implement", "next batch", "continue", "quick change", "small tweak",
  "run the boundary", and before claiming a batch done. Gears: batch (the
  current PLAN.md batch — the default), boundary (batch end: gates, gap
  check, learnings, ledger commit, stop), finish (all green: QA, ADRs, final
  commit), quick (small scoped changes; gates and Verified-by mandatory).
  Bugs: craftsman-fix; new behavior: craftsman-spec first. Applies only
  inside a Craftsman project (craftsman.toml present); otherwise offer
  craftsman-init and stop.
license: MIT
compatibility: Requires the craftsman CLI on PATH and git.
---

# Craftsman Implement

You turn red scenarios green, production grade, in continuous context. Read `references/craftsman-conventions.md` once per session first, then the stack file matching craftsman.toml (`references/stack-<stack>.md`) before the first line of code.

## Routing

| Signal | Gear | Load |
|---|---|---|
| Working a planned batch; "implement", "next batch" | `batch` (default) | stack file |
| Batch scenarios all green; "run the boundary" | `boundary` | `references/boundary.md` |
| Every SPEC.md scenario green; "finish", "wrap up" | `finish` | `references/finish.md` |
| Small scoped change, no new behavior; "quick change" | `quick` | `references/quick.md` |

Scale routing (conventions) decides between `quick`, craftsman-fix, and spec-first — check it before defaulting to `batch`.

## batch (default)

Per task, in order:

1. **Ground**: fetch docs for every API the task touches (`craftsman docs`). No source, no code.
2. **Aim**: `craftsman verify --impact` — see which scenarios and tests this change can affect. Confirm the target scenarios are red (`craftsman verify --scenario "…"`); a target that's already green means the plan is stale — route to craftsman-plan.
3. **Build**: the minimum that makes the target scenarios green, in the stack's idiom (stack file). New dependency? Run the vetting protocol first — `references/dependencies.md`.
4. **Verify**: `craftsman verify --impact` again. Red → recovery budget (conventions): classify, spend, stop at the cap with a report and a drafted ADR.
5. **Refactor while green**: shape the code, re-verify. Emergent unit/integration/property tests per `references/testing.md` — test what has logic, skip what doesn't.

If a task reveals the plan is wrong, update PLAN.md (via craftsman-plan revise) before continuing. When the batch's scenarios are all green, go to `boundary` — never start the next batch without it.

## boundary / finish / quick

Each is a strict checklist — load its reference and execute in order, no steps skipped, no reordering. Common shape: verify everything, gate everything (`craftsman check-all`), extract what was learned, commit through `craftsman commit`, stop and report. `finish` adds ADR consolidation (human-gated), stale-ADR detection, and the AGENTS.md accuracy check. `quick` is the whole loop compressed for a small change — gates and ledger, no ceremony.

## Never

- Never write code against an API you haven't grounded in current docs this session.
- Never declare green from reading code — exit codes only.
- Never start the next batch after a boundary without the human saying so.
- Never keep a stub, TODO, or "good enough for now" — production grade is the only grade.
- Never let `quick` grow behavior — the moment it does, stop and route to craftsman-spec.
- Never spend past the recovery budget — stop, report, draft the ADR.
