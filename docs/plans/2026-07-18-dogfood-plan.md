# Dogfood Program — Craftsman on itself and beyond

Companion to `2026-07-17-cli-implementation-plan.md`. That plan holds scenario-backed
batches with `craftsman verify` success lines; this one holds the dogfood work that
produces *evidence*, not scenarios. Items graduate: a dogfood finding that implies new
CLI behavior routes to craftsman-spec and lands in the implementation plan as a batch.

## What the dogfood has already taught (harvested 2026-07-18)

Recorded here so the learnings are inspectable in one place; each is also in the
ledger (`git log --grep="Learned:"`).

1. **Delta-file pattern** (bcb7dc2) — approved-but-unimplemented scenarios must live
   outside the executed spec (SPEC.delta.md) until the boundary merge, or the commit
   gate refuses the spec-only commit. Candidate CLI nicety harvested below.
2. **Plan revise chicken-and-egg resolves** (2f725d7) — a batch that needs scenarios
   which need a spec delta makes the delta task 1; the flow holds.
3. **Trigger-craft asymmetry** (efdc955) — Superpowers' over-trigger-cheap policy
   inverts for expert users with standing artifacts; wording harvested, policy not.
4. **Debug-binary drift** — every dogfood run so far used `cli/target/debug/craftsman`;
   the real install path (install.sh → release → setup) was never exercised. Fixed
   structurally by Batch 10's redeploy task; the program below never invokes the
   debug build again after v0.2.0 lands.
5. **Recover discipline scales** — the whole-surface retro-spec (a sanctioned
   exception to no-backfill) held the verified-only bar: 52/61 drafted scenarios
   survived citation; 9 became gaps instead of guesses (→ Batch 12).

The first *external* dogfood — craftsman-web, a copied-and-rebranded sibling
brought under the installed v0.2.0 — produced its own ledger
(`../craftsman-web/docs/dogfood/ledger.md`). Harvested 2026-07-18; every
finding is routed, none rides on memory:

6. **`bunx` puts the network in the verdict path** (ledger 2, severe) — the
   cucumber-js adapter's `bunx` auto-installed the runner, mutated the project's
   bun.lock, and pulled a dependency-confusion stub. → Batch 13 fix; AGENTS.md
   gains the no-install-in-verdict-path hard constraint.
7. **init's ts spec default is dead on arrival** (ledger 1) — `spec = "SPEC.md"`
   is invisible to cucumber-js's `features/**/*.feature` discovery → exit 4 on
   first verify. → Batch 13 fix (per-stack `.feature` scaffold).
8. **`craftsman commit` cannot make a repo's first commit** (ledger 6) — unborn
   HEAD breaks `--changed`. → Batch 13 fix (empty-tree diff fallback).
9. **install.sh is not idempotent + PATH probing** (ledger 5) — `cargo install`
   without `--force`; non-interactive shells miss `~/.cargo/bin`. → Batch 13 fix
   (script) + Batch 14 (skill-side probe note).
10. **The greenfield/brownfield fork rested on phrasing** (ledger 3 + 4b) — a
    copied tree got greenfield-strict gates; inherited findings blocked until a
    manual `gate baseline`. → ADR-006 entry doctrine: `init` refuses non-empty
    trees; new `import` gear (audit-first, explicit debt disposal) → Batch 15.
11. **Existing QA lived outside the system** (ledger 1b) — the site's real
    acceptance (`bun run qa`) had no place in the contract; verify was satisfied
    by a token walking skeleton. → ADR-006 §5: `[gates.qa]` command gates
    (verify stays always-strict BDD; the external-verify-adapter idea is
    rejected) → Batch 16.
12. **Doctor is blind to pinned gate tools** (ledger 4) — only git/cargo are
    checked; two gates silently unrunnable on a fresh machine. → Batch 14.

## Harvested CLI niceties (route through craftsman-spec before any code)

- `spec lint --delta` — lint SPEC.delta.md scenarios against the main spec (name
  collisions, gherkin validity) without admitting them to the executed set.
- `spec merge-delta` — mechanical boundary merge of an approved delta file, so the
  single-writer rule covers the merge too. (Today it is a hand edit — the one spec
  write the CLI does not mediate.)

Neither starts before Batch 12 closes; both need a human-approved delta.

## Phase D1 — Redeploy and re-enter (after Batch 10 tags v0.2.0)

- Install v0.2.0 via `sh install.sh` (release binary path, not cargo); `craftsman setup`;
  `craftsman doctor` — all from the installed binary on a clean PATH.
- Confirm skill copies in ~/.agents/skills match the v0.2.0 embedded set (setup
  reports every skill up to date on second run).
- From then on: this repo's own batches (11, 12) run under the *installed* binary.
- Success: doctor exit 0 from `command -v craftsman` resolving outside the repo.

## Phase D2 — Trigger matrix (after D1)

The 50-query matrix from the trigger-craft research (Appendix A), run per harness:
Claude Code, Codex, Gemini CLI, Goose, Pi, OpenCode, Xcode 27.

- Score each query: correct skill+gear routed / wrong skill (false positive) /
  no trigger (false negative); record per-harness tables in
  `docs/research/2026-07-XX-trigger-matrix-results.md`.
- Threshold from the research: >90% correct routing, zero destructive-gear false
  positives (init/adopt must never trigger from a near-miss).
- Failures feed description rewording — skills change, conventions do not.

## Phase D3 — Greenfield init dogfood (parallel with D2)

`craftsman init` on a scratch repo per stack (swift, python, typescript, rust, bash):
scaffold → first spec → one batch → boundary, using only the skills.

- Watch for: doctor 5/5 immediately after init; first `craftsman commit` succeeds
  without hand-holding; conventions announce-at-start fires in each harness.
- Success per stack: one feature green through the full spec→plan→implement loop.

## Phase D4 — Real-app deferred proofs (needs the first real app project)

The two ADR-005 deferrals that require a genuine application:

- Live `performAccessibilityAudit` through the a11y gate on a real Xcode 27 app
  (the XCTest stub is write-once scaffolded; the audit needs a launchable target).
- k6 live artifact through the perf gate against a running service.
- Success: both gates produce a real red before their first green (honest-probe rule).

## Phase D5 — Eval protocol (after D2+D3 stabilize the descriptions)

8 tasks × 3 conditions (bare agent / conventions only / full craftsman) per the
production-grade research; measures the system's claimed edge, not vibes.
Results land in docs/research/ and drive the v0.3 roadmap.

## Phase D6 — Import dogfood (deferred by the human 2026-07-18)

craftsman-web is now a live production site — it stays as-is. The import
gear gets its dogfood on the next incoming project instead (any copied
sibling or vendored tree). The exercise, whenever that project appears:

- `craftsman import` on a fresh copy of the tree: detect → scaffold → audit →
  QA conversion; compare the flaw inventory against the hand-recorded baseline
  from the first pass.
- Convert `bun run qa` (build · i18n-parity · assets · links · a11y · seo ·
  agent-skills) into declared `[gates.qa]` gates; `check-all` and the commit
  hooks then carry the site's real acceptance.
- Success: the imported project's `Verified-by:` trailer names its qa gates;
  its dogfood ledger records the delta against the craftsman-web experience.

## Standing rules for the program

- Every dogfood session runs under the installed release binary (post-D1).
- Findings are Learned:/Rejected: trailers first, program-doc updates second.
- A finding that implies CLI behavior change → craftsman-spec, never a quick patch.
- This document is agent-owned like PLAN.md; revised when a phase completes.
