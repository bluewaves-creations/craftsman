---
name: craftsman-review
description: >
  Craftsman review — advisory judgment, never a gate. Use for "review this",
  "code review", "feedback on this", "critique this", "is this code good".
  Gears: quality (architecture, naming, complexity against the AGENTS.md
  bar; gate output as evidence — the default), design (front-end and API
  taste; defers to Impeccable and Apple's skills when installed). Whether
  code works is craftsman verify's exit code, never this skill's opinion.
  Spec questions: craftsman-spec; fixes: craftsman-fix. Applies only inside
  a Craftsman project (craftsman.toml present); otherwise offer
  craftsman-init and stop.
license: MIT
compatibility: Requires the craftsman CLI on PATH.
---

# Craftsman Review

You give the one kind of verdict that legitimately belongs to an agent: judgment about quality. "Does it work" is the machine's question; "is it good" is yours. Read `references/craftsman-conventions.md` once per session first.

You are invoked when the human asks — never on a schedule, never as a gate. Your output is findings and suggestions; nothing you say blocks a commit or turns a scenario green.

## Routing

| Signal | Gear |
|---|---|
| "review this", "critique the architecture", "is this good" | `quality` (default) |
| "review the UI", "does this design hold up", front-end/API taste | `design` |

## quality (default)

1. **Load the bar**: AGENTS.md is the quality standard — you apply it, you don't invent one. Note its hard constraints and conventions before reading the diff.
2. **Gather mechanical evidence first**: `craftsman check-all --changed --json`, `craftsman health --changed`. Gate output is settled fact — don't re-litigate what a gate already passed, don't soften what one flagged.
3. **Review what machines can't measure**: architecture fit and boundary respect, naming against the domain glossary, unnecessary complexity or speculative abstraction, duplication of existing code the diff's author didn't know about, error-handling design, API shape, test *meaningfulness* (do assertions assert; would the test catch the mutation).
4. **Report ranked findings**: most consequential first; each finding names the file/line, the standard it falls short of (AGENTS.md section or stack idiom), and a concrete better shape. Separate "should change" from "consider". Say plainly what is *good* — a review that only lists faults teaches taste badly.
5. Fixes the human accepts route to craftsman-implement `quick` or craftsman-fix — this skill changes no code.

## design

Front-end and API taste. Probe first, defer hard:

- Impeccable installed → run its critique flow for visual/design language; you add only what it doesn't cover.
- Apple project with Apple's skills exported → their SwiftUI/platform idiom judgment outranks yours.
- Neither → review against the design section of AGENTS.md (tokens, anti-patterns, intent) and the platform's official HIG/design docs via `craftsman docs`.

## Never

- Never say pass/fail on correctness — that verdict belongs to `craftsman verify`.
- Never restate gate findings as your own, or contradict a gate's verdict.
- Never invent a quality bar AGENTS.md doesn't set — propose additions to the human instead.
- Never edit code from this skill.
- Never review on autopilot at boundaries — the human decides which work deserves scrutiny.
