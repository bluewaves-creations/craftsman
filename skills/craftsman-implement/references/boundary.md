# The Batch Boundary

Loaded when a batch's target scenarios have all gone green. This is a strict checklist: execute in order, skip nothing, reorder nothing. The boundary exists so that nothing broken, unhealthy, or unrecorded survives a batch.

## The checklist

1. **Full verification.** `craftsman verify` — all scenarios, not just the batch's. A regression found here is a `regression`-class failure: 2 attempts to fix without breaking the new work, then stop (conventions, recovery budgets).

2. **All gates.** `craftsman check-all`. Per failing gate: read the output, fix, re-run — at most 3 rounds per gate. A gate still red after 3 rounds → stop the boundary, report, do not proceed and do not commit.

3. **Gap pass.** Route to craftsman-plan `gap`: does every remaining red scenario still have a batch, and did this batch teach anything that invalidates a planned approach? Plan revisions happen now, while the learning is fresh.

4. **Extract.** `craftsman extract` — write to `.craftsman/session/`:
   - decisions made this batch and why (with the rejected alternatives),
   - approaches that failed and the reason,
   - files created/modified, scenarios that changed state,
   - open questions for the human.

   Extract only what cannot be re-derived from disk or git. Tool output, file contents, error text — all re-obtainable; don't extract them.

5. **Commit.** `craftsman commit` — type `feat(batch-N)`, trailers: `Scenarios:` (mandatory), `Learned:`/`Rejected:` (whenever there is one worth a future session reading), `Ref:` to SPEC.md/PLAN.md. The CLI refuses if gates are red and writes `Verified-by:` itself — if it refuses, you are back at step 2, not looking for a workaround.

6. **Stop and report.** Green/red scenario counts, gate results, gap findings, plan revisions, learnings. Then wait. The human says "next" — you never start the next batch on your own.

## If context is long

After step 5, this is the safe moment for the harness to compact: everything durable is on disk. If you resume from a compaction, read `.craftsman/session/index.md` first — it is the "where was I" briefing.

## Never

- Never run a partial boundary ("just committing for now") — the checklist is atomic.
- Never batch two boundaries together after the fact; each batch gets its own commit and report.
- Never carry a red gate across a boundary "to fix in the next batch".
