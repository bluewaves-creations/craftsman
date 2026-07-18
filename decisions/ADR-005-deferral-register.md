# ADR-005: Batch 9c deferral register — everything still open after debt zero

Status: Accepted (human, 2026-07-18) · Date: 2026-07-18 · Evidence: the Batch 9 plan's
honest-undone register, the Batch 9c close-out, and the observed limits
recorded in the ledger.

This ADR is the final form of the plan's honest-undone register: after
Batch 9c every gate the repo enables runs strict with an empty baseline,
and what remains open is exactly the list below. Each line carries its
rationale; the human approves or amends this register (Proposed until
then). The plan's registers now point here instead of tracking items
inline.

## Remote-gated (waiting on the human's repository decision)

The org/name/visibility of the GitHub remote is a human call the agent
must not invent. Until it exists:

1. **GitHub remote creation + first CI run** — CI has never executed on a
   real runner; the workflow is committed and the local `check-all` is the
   stand-in verdict.
2. **`dist generate` release workflow** — cargo-dist requires a repository
   URL to emit `release.yml`; the dist config is committed and pinned
   (cargo-dist 0.32.0).
3. **`craftsman update` real path (axoupdater)** — needs GitHub Releases
   to exist; `update` currently reports the honest guidance and refreshes
   skills from the embedded payload.
4. **swift-linux CI live probe** — the job is desk-verified and pinned
   (setup-swift v2.4.0, ubuntu-24.04, Swift "6.2"; fallback
   vapor/swiftly-action@v0.2.1) but stays commented until one canary run
   on the real remote settles open issue #677.

## Deferred with cause (not remote-gated)

5. **k6 live artifact** — the k6 perf parser is unit-tested against a
   schema-doc-constructed sample only; no k6-shaped target project exists
   in this repo to produce a real `--summary-export`. First dogfood
   project with an HTTP surface closes it.
6. **Live `performAccessibilityAudit()`** — the Apple a11y gate is
   plumbing-proven on a real xcresult, but the audit itself needs an app
   host: XCUITest cannot run against a SwiftPM package (observed, Batch
   9a). First real `.xcodeproj` app target — the first dogfood app —
   closes it.
7. **Sub-lettered batch rollup in `spec status`** — the plan parser reads
   `## Batch N` numerically; 9a/9b/9c sub-batches don't get per-letter
   rollup rows. Cosmetic: the scenarios still roll up under batch 9's
   Scenarios lists, and verify `--batch` selection is unaffected.
8. **pydantic "latest" version caching** — the objects-inv dogfood pins
   nothing; upstream stamps its inventory `0.0.0`, so `docs status` shows
   a meaningless cached version for it. Upstream artifact reality, not a
   craftsman defect; revisit only if a version-bearing inventory source
   appears.

## Decided and recorded (no action open)

9. **Mutate at boundaries, not per commit** — `[gates] mutate` stays off
   as a commit gate (a tiny-diff run measured ~30s wall); `craftsman
   mutate` runs on demand at batch boundaries. Recorded in
   craftsman.toml's `[gates]` comment since Batch 6b; restated here so the
   register is complete.

## Consequence

With this register approved, the honest-undone register in
`docs/plans/2026-07-17-cli-implementation-plan.md` is empty — items are
either done and proven, or listed above with an owner condition (the
remote decision, or the first dogfood project). v0.2.0 stays reserved for
CI-green on a real remote; the local-complete state is tagged v0.2.0-rc1.
