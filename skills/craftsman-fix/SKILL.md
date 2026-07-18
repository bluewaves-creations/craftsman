---
name: craftsman-fix
description: >
  Craftsman bug fixing — diagnose before proposing any fix. Use when tests
  fail, something crashes, or behavior surprises: "fix this bug", "tests are
  failing", "it crashes", "why is this failing", "why does X happen",
  "scenario went red", "regression". Gears: diagnose (reproduce, isolate,
  report — the mandatory default), fix (root-cause test + minimal fix, one
  commit), improve (separate refactor commit). Small non-bug changes:
  craftsman-implement quick; new behavior: craftsman-spec. Applies only
  inside a Craftsman project (craftsman.toml present); otherwise offer
  craftsman-init and stop.
license: MIT
compatibility: Requires the craftsman CLI on PATH and git.
---

# Craftsman Fix

You fix root causes, not symptoms, and you leave the codebase healthier than you found it. Read `references/craftsman-conventions.md` once per session first.

The three gears are phases and they run in order. `diagnose` is the default and mandatory first — there is no path to `fix` that skips it.

## diagnose (default)

1. **Reproduce mechanically.** One command that triggers the failure (`craftsman verify --scenario "…"`, a failing test, a script). Can't reproduce → gather more data; a diagnosis without reproduction is a hypothesis wearing a costume.
2. **Isolate.** `git bisect` for regressions; trace data flow from input to failure point; compare working vs broken paths and list every difference.
3. **Pre-action gate** (conventions): check `decisions/index.md` and `git log --grep="Rejected:"` for this area — the fix you're about to propose may already be recorded as a failure.
4. **Hypothesize.** One clear root-cause hypothesis and the smallest change that would verify it.
5. **Report before fixing**: root cause, introducing commit (if found), affected scope, proposed approach. For architectural causes, draft the ADR now. Routine bugs proceed to `fix`; anything surprising waits for the human.

## fix

One commit, three things, nothing else:

1. **Failing test for the root cause** — not the symptom. It fails before the fix, passes after, and its docstring/comment names the root cause and references the diagnosis.
2. **Minimal fix** — the root cause only. No while-I'm-here improvements, no drive-by renames.
3. **Full verification** — `craftsman verify` (all) + `craftsman check-all --changed`; then `craftsman commit` (type `fix`, trailers carry root cause and `Ref:`).

## improve

Separate commit, only after `fix` is green and committed:

1. `craftsman health --changed` on the files the fix touched.
2. Health degraded by the fix → refactoring is mandatory. Already low (< the configured bar) → recommended; confirm scope with the human.
3. Refactor in small steps, `craftsman verify` after each; commit with type `refactor`, health-before/after in the body.

If the refactor breaks anything, it reverts alone — the fix stays.

## Never

- Never write fix code before a reproduced, reported diagnosis.
- Never fix the symptom to get green — a try/catch around a null pointer is a cover-up, not a fix.
- Never mix fix and refactor in one commit.
- Never leave a fixed bug without its root-cause regression test.
- Never retry past the recovery budget — stop, report, draft the ADR.
