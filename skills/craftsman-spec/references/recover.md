# Recover

Brownfield truth recovery: pin what the system actually does with characterization tests, let the human judge each pinned behavior, and admit only machine-verified scenarios to SPEC.md.

## Scope discipline

Recover hotspots and critical paths only — the areas that are churning, revenue-critical, or about to change. Never backfill the whole codebase: documentation for code not under active change goes stale because nothing forces it to track reality, and the budget it burns buys no safety. If an area is neither hot, critical, nor about to be touched, leave it unrecovered and say so.

## The sequence

Run these steps in order; no step starts before the previous one finishes.

### 1. Find the seam

Identify the narrowest point where the behavior can be observed and exercised in isolation — a function boundary, a CLI invocation, a request handler. If no seam exists, propose the minimal dependency-breaking change to create one and get human approval before touching code. No seam, no characterization.

### 2. Generate characterization tests

You generate them at scale; that is the part agents made cheap. The test records what the code does today — including behavior that looks like a bug. Per stack:

| Stack | Tool | Baseline |
|---|---|---|
| Swift | swift-snapshot-testing | recorded snapshots per assertion |
| Python | syrupy or ApprovalTests.Python | snapshot dirs / `.approved.` files |
| TypeScript | Vitest snapshots | `__snapshots__/`, inline snapshots |
| Rust | insta (`cargo insta review`) | `.snap` files, interactive review |
| CLI / bash | golden-master diff scripts | recorded stdout/stderr/exit codes |

Never assert what the code *should* do at this stage — record what it *does*, and annotate anything suspicious for step 3.

### 3. Human approves every snapshot — irreducible

Present each snapshot for one of two verdicts:

- **Pin as intended** — the behavior is truth; the snapshot becomes part of the behavioral baseline.
- **File as bug** — the behavior is a defect; it gets a tracked bug ID and is *not* pinned. Fixing it is later `craftsman-fix` work, never part of recovery.

This judgment cannot be delegated to you or to the machine: only the human knows whether rounding-half-down is a contract users depend on or a five-year-old mistake. Batch the review to respect their time, but never skip a snapshot and never approve one yourself.

### 4. Draft scenarios from approved behavior only

Write Gherkin (per `references/gherkin-authoring.md`) for pinned snapshots only. Bugs that were filed get no scenario until fixed; unapproved snapshots get nothing.

### 5. Label every recovered claim

- `verified` — backed by an approved, passing characterization test. Cite it.
- `inferred` — your reading of the code, unexecuted. An LLM's reading of code is an opinion, not a fact.
- `gap` — unknown behavior that needs human input or a seam that doesn't exist yet.

### 6. Only `verified` enters SPEC.md

Verified scenarios go under a "Current behavior (recovered)" section, each citing its characterization test, and land only with human approval — the human owns SPEC.md. `inferred` scenarios stay in the draft, clearly labeled, until a test promotes them. `gap` items become tracked work items (GAP-NNN) listed for the human — a gap is a work item, never silence.

```gherkin
# SPEC.md — Current behavior (recovered)
Scenario: Expired session redirects to login   # confidence: verified
  # Verified-by: tests/characterization/test_session.py::test_expired_redirect
  Given a session idle for more than 30 minutes
  When the user requests any authenticated page
  Then they are redirected to login with the original URL preserved
```

## The commit

Commit through `craftsman commit` with type `retro-spec`, one commit per recovered area:

```
retro-spec(auth): pin session expiry behavior

12 characterization tests added at the SessionMiddleware seam.
2 snapshots filed as bugs (BUG-114, BUG-115), not pinned.
3 scenarios promoted to SPEC.md as verified; 1 gap tracked (GAP-007).

Scenarios: Expired session redirects to login; Active session refreshes idle timer
Learned: expiry uses server clock, not token issue time — pinned deliberately
Ref: SPEC.md "Current behavior (recovered)"
```

`Verified-by:` is written by the CLI only when gates actually passed — never by hand. The exit code of `craftsman verify`, not your reading, is the only proof that recovered truth is true.
