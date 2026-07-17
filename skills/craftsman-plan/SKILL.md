---
name: craftsman-plan
description: >
  Craftsman planning — batches SPEC.md scenarios into a lean PLAN.md the work
  follows. Gears: batch (group red scenarios into 2–4-scenario batches with
  mechanical success criteria — the default), revise (replan from what the
  last batch taught), gap (check remaining batches still cover all remaining
  red scenarios). Use for "plan the batches", "update the plan", "what's
  next", "replan". Use the harness's native plan mode for the thinking;
  PLAN.md is the durable artifact. For execution use craftsman-implement.
  Applies only inside a Craftsman project (craftsman.toml present); otherwise
  offer craftsman-init and stop.
license: MIT
compatibility: Requires the craftsman CLI on PATH.
---

# Craftsman Plan

You own PLAN.md: the dynamic, batched roadmap from red scenarios to green. Read `references/craftsman-conventions.md` once per session first.

The spec doesn't move; the plan does. A plan is good when each batch is small enough to execute without re-reading the plan, and when its success line is mechanical.

## Routing

| Signal | Gear |
|---|---|
| No PLAN.md yet, or new spec content; "plan the batches" | `batch` (default) |
| A batch just completed; "update the plan", "replan" | `revise` |
| Boundary protocol step, or "are we still covered?" | `gap` |

## batch (default)

1. `craftsman spec status` — inventory red scenarios.
2. Group into batches of 2–4 *related* scenarios (shared model, shared module, shared risk). Order by dependency, then by learning value: the batch that teaches the most about the architecture goes first.
3. Where the harness has a native plan mode, do the thinking through it; persist the outcome as PLAN.md — the durable, harness-neutral artifact.
4. Write each batch as:

```markdown
## Batch 2: Completion flow
Scenarios:
- Complete a todo item
- Completed items appear in history
Tasks:
- Add completion state to model
- Implement completion endpoint
- Wire step definitions
Success: craftsman verify --batch 2 exits 0
```

5. `craftsman plan lint` — every listed scenario must exist in SPEC.md, no red scenario unassigned.

PLAN.md contains batches, tasks, and success lines — never architecture prose, never code.

## revise

Observation-grounded, at batch boundaries only: given what the completed batch taught (`Learned:`/`Rejected:` trailers, gate results), are the remaining batches still the right approach? Remove the completed batch, fold in learnings, consolidate what got simpler, split what got bigger than a context comfortably holds. Record consequential re-orderings in the commit body at the boundary.

## gap

Re-read SPEC.md against PLAN.md: does every remaining red scenario appear in exactly one remaining batch? `craftsman plan lint` reports the mapping; your job is the judgment call it can't make — scenarios that are *nominally* covered but whose batch approach the last boundary invalidated. Surface every gap as a plan change, never silently.

## Never

- Never edit SPEC.md from this skill — gaps in the spec route to craftsman-spec.
- Never write a batch whose success line isn't a `craftsman verify` invocation.
- Never plan more than the next 3–4 batches in task-level detail — beyond that, a scenario list suffices; detail decays.
- Never mark a batch done — exit codes at the boundary do.
