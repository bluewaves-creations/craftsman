# The Finish

Loaded when every scenario in SPEC.md is green. Finishing is not stopping — it is the pass that turns a working batch series into a finished piece of work.

## The checklist

1. **Full QA.** `craftsman verify` (all) and `craftsman check-all` — every enabled gate, whole project, not `--changed`. Failures here get the same bounded improvement loops as a boundary (3 rounds per gate), then stop and report if still red.

2. **Delta merge.** Any spec deltas this work implemented (craftsman-spec `delta`) merge into SPEC.md's main body now — human approves the merged spec.

3. **ADR consolidation (human-gated).** Group related ADRs from this work; propose merges ("ADR-003, 005, 009 are all data-layer → one `data-architecture.md`, tried-and-failed history compressed to terse lines"). Propose only — the human approves every merge. Then `craftsman adr index` to regenerate the index.

4. **Stale-ADR detection.** `craftsman adr stale` — for each active ADR whose cited files have moved on significantly, report it: "confirm or supersede?" The human decides; you never silently rewrite a decision record.

5. **AGENTS.md accuracy check.** Did this work change anything AGENTS.md asserts — commands, structure, constraints? Propose the minimal correction (observed facts only, budget still applies). Stale context files actively mislead the next session.

6. **Documentation.** Update or generate the docs the project's AGENTS.md says it keeps (README, API docs, CHANGELOG). No invented doc obligations.

7. **Final commit and report.** `craftsman commit` referencing the completed scenarios; report the full green board, consolidations made, anything flagged. Branch/PR mechanics only if AGENTS.md calls for them.

## Never

- Never finish with a gate in `baseline` mode showing ratchet regressions — the baseline must be at or below where the work started.
- Never consolidate ADRs without human approval, or delete the tried-and-failed history while consolidating.
- Never leave SPEC.md with an unmerged delta section after the work it specified is green.
