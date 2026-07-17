# Adaptive Dynamic Planning: Research & Alternatives

> How agents should handle struggle, plan revision, improvement loops, and QA gates — evaluated against the 2026 landscape of harness engineering, self-correction patterns, and replanning research.

---

## The Question

Craftsman Dev currently says "PLAN.md adapts after each batch" and "stuck: three attempts, then ADR and stop." But the actual mechanics of adaptation are underspecified. When exactly should the plan revise? How should improvement loops work? What stops the agent from declaring premature victory? What does the 2026 research say about agents that struggle?

## The Core Failure Mode: Premature Completion

The single most important finding from 2026 harness research, from the Intelligent Internet team's RALPH-to-Zenith study:

> "The failure mode shows up immediately. On long-horizon work, the agent ships a plausible draft, runs the checks it chose, writes a confident summary, and stops while real requirements are still missing. We call this premature completion."

This is the failure mode Craftsman Dev's Gherkin architecture was designed to prevent. The agent can't declare victory because the test runner declares victory — or doesn't. But premature completion has a subtler cousin: **premature plan satisfaction**. The agent completes all planned tasks, all targeted scenarios pass, and the batch boundary triggers — but the plan missed something the spec actually requires. The tasks were completed; the work wasn't.

The RALPH pattern (Repeated Agent Loop for Persistent Harnesses) offers the simplest cure: after each session, force the agent to re-read the original requirement and ask "what is still missing between the current state and the spec?" This gap-finding pass costs tokens but catches the gaps that task completion misses.

The critical finding: **Plan-RALPH (static plan + RALPH loop) underperformed planless RALPH on every task tested.** A static upfront plan became a liability because it locked in early-stage assumptions. The iterative loop spent its budget executing a frozen blueprint rather than closing the gaps that actually mattered.

This validates Craftsman Dev's "PLAN.md is dynamic" principle — but pushes it further. The plan should not merely be *editable*. It should be *actively questioned* at every batch boundary.

## The Five Patterns That Matter

### 1. Plan-Execute-Replan (the foundation)

The standard architecture separates planning from execution and adds a replanning step after each execution phase. The replanner receives the original objective, the current plan, and the history of executed steps with their outcomes, then decides: continue, revise remaining steps, or declare completion.

Craftsman Dev already implements this at batch boundaries. The upgrade from the research: replanning should be *observation-grounded*. The replanner doesn't just ask "what's next?" It asks "given what we learned from this batch, are the remaining batches still the right approach?"

A concrete example: batch 1 reveals that the chosen data model doesn't support a scenario in batch 3. Without observation-grounded replanning, the agent would continue to batch 2 and discover the problem later. With it, PLAN.md updates immediately: batch 2 gets a new task to refactor the data model, and batch 3's tasks are revised accordingly.

### 2. Improvement Loops (generate → verify → fix → re-verify)

The generate-verify-fix cycle is the mechanical core of quality enforcement. The 2026 research distinguishes three types:

**Rules-based feedback loops** — the agent generates code, runs linters/type-checkers/tests, reads the errors, fixes them, and re-runs. This is the most reliable form of self-correction because the feedback signal is deterministic. The agent reads an error message and chooses a different approach — "the one form of self-correction that reliably works" (harness engineering guide).

**Verification-grounded loops** — the agent generates code, runs the test suite (Gherkin scenarios), reads failures, fixes the implementation, and re-runs until green. This is exactly Craftsman Dev's per-task workflow. The research confirms it's the right architecture.

**Reflexion loops** — the agent generates a verbal self-critique after failure, stores it as a lesson, and uses it to condition the next attempt. Reflexion improved GPT-4's coding benchmark from 80% to 91%. However, a 2026 paper ("Honest Lying: Understanding Memory Confabulation in Reflexive Agents") found that Reflexion assumes the reflection step produces causally correct diagnoses — and this assumption fails systematically when feedback is binary (pass/fail). The agent can write a plausible but wrong lesson, then reinforce that wrong lesson on subsequent attempts.

**Implication for Craftsman Dev:** Use rules-based and verification-grounded loops (deterministic feedback). Avoid Reflexion-style verbal self-critique as a primary mechanism. When the agent struggles, the correct response is not "write a lesson about what went wrong" — it's "read the error message, consult the docs, try a different approach." If that fails three times, escalate to the human via ADR. The ADR *is* the lesson — but it's human-gated, not agent-generated.

### 3. Gap-Finding (the premature completion cure)

The RALPH insight: after each batch, the agent should re-read SPEC.md and ask "what scenarios remain red that the current plan doesn't address?" This is different from "what's the next batch?" It's a deliberate check for gaps between the spec and the plan.

The gap-finding pass costs roughly one additional LLM call per batch boundary. The return is catching scenarios that were assumed to be covered by a later batch but actually aren't.

For Craftsman Dev, this integrates naturally at step 2 of the batch boundary: after running full verification, before reporting to the human. The agent reports not just "which scenarios are green and which are red" but also "are the remaining batches still aligned with the remaining red scenarios?"

### 4. Recovery Budget (bounded retry)

The self-healing orchestrator research (arXiv 2606.01416) formalizes what Craftsman Dev already intuits with "stuck: three attempts":

> "Recovery should be targeted and bounded: the system should preserve useful execution state when possible, choose recovery actions appropriate to the inferred failure class, and avoid unbounded retry or replanning loops."

The key concepts:

**Recovery budget** — a fixed number of retry attempts per failure class. Craftsman Dev uses a flat 3. The research suggests this can be more nuanced: 3 for implementation failures (wrong approach), but 1 for environment failures (missing dependency, API down) where the fix is outside the agent's control.

**Failure classification** — not all failures are the same. The self-healing orchestrator distinguishes: tool timeout (retry with backoff), stale context (refresh documentation), malformed output (repair), contradictory evidence (cross-check), wrong approach (replan). Different failures need different recovery actions.

**Escalation** — when the budget is exhausted, the accumulated failure trace is the most useful artifact the agent produced. Craftsman Dev's "draft an ADR" is exactly this: the failure trace becomes institutional knowledge. The Reflexion research confirms: "If the retry cap is hit without success, don't just fail — escalate. The accumulated reflections are often the most useful thing the agent produced, even in failure."

### 5. Stopping Conditions (when is "done" actually done?)

The Zenith harness introduces explicit stopping decisions. At every boundary, the orchestrator decides: continue, replan, add a worker, add a tester, reset strategy, move to the next milestone, or stop. The key insight: **stopping is an active decision, not the absence of more work.**

For Craftsman Dev, the stopping condition is mechanical: all scenarios in SPEC.md are green, all QA gates pass (lint, type-check, health), no regressions detected. This is already stronger than most agentic systems because the human defined "done" (SPEC.md) before the agent started working. The agent cannot redefine success.

The gap the research surfaces: what about *quality of solution*? All scenarios green doesn't mean the implementation is good. It means it works. The code review agent fills this gap — but only if invoked. The batch boundary could include a lightweight quality signal: CodeHealth score as a numerical gate, not an opinion.

## What Craftsman Dev Should Adopt

### Enhanced Batch Boundary Protocol

The current batch boundary (stop → verify → report → compress → refine plan → wait) should be augmented:

```
Batch N scenarios all green
    │
    ├── 1. Run full verification (all scenarios, catch regressions)
    │
    ├── 2. Run QA gates (lint, type-check, health score)
    │       └── If any gate fails → improvement loop:
    │           fix → re-run → repeat (max 3 per gate)
    │           └── If still failing → report to human, do not proceed
    │
    ├── 3. Gap-finding pass
    │       Re-read SPEC.md. Are remaining red scenarios still
    │       covered by remaining batches in PLAN.md?
    │       └── If gap found → revise PLAN.md before reporting
    │
    ├── 4. Commit with structured message
    │
    ├── 5. Compress context
    │
    └── 6. Report to human:
            - Green/red scenario count
            - QA gate results
            - Gap-finding results
            - Plan revisions (if any)
            - Lessons learned → draft ADR if architecturally significant
```

### Structured Recovery Protocol

Replace the flat "3 attempts then stop" with failure-aware recovery:

**Implementation failure** (wrong approach, logic error):
- Attempt 1: read error, consult docs, fix
- Attempt 2: try alternative approach
- Attempt 3: simplify — reduce scope to minimum passing implementation
- Exhausted: stop, report what was tried, draft ADR

**Environment failure** (missing dependency, API down, tooling issue):
- Attempt 1: diagnose and fix environment
- Exhausted: stop immediately, report — this is outside the agent's control

**Specification failure** (scenario is ambiguous or contradictory):
- Attempt 1: re-read spec, attempt the most literal interpretation
- Exhausted: stop, flag the specific ambiguity to the human

**Regression failure** (new code breaks existing green scenario):
- Attempt 1: identify the regression, fix without breaking the new scenario
- Attempt 2: if conflict is fundamental, stop — the spec may need revision
- Exhausted: stop, report the conflict — only the human can resolve spec-level conflicts

### Dynamic Plan Revision Rules

PLAN.md revision should follow explicit rules:

**After each batch:**
1. Remove completed batch
2. Run gap-finding against SPEC.md
3. If a completed task produced learnings that invalidate future tasks, revise those tasks
4. If a new approach was discovered that simplifies future batches, consolidate

**Never revise:**
- SPEC.md (only the human changes acceptance criteria)
- Completed commits (history is immutable)

**Always revise when:**
- A task revealed that a future task's approach won't work
- The estimated scope of a remaining batch exceeds what can be held in context
- A remaining batch's target scenarios are already partially green from earlier work

### Improvement Loop Architecture

Two distinct loops, triggered differently:

**Inner loop (per task, automatic):**
```
write code → run scenario → red?
    → read error → consult docs → fix → re-run
    → still red after 3? → stop task, report
```

**Outer loop (per batch boundary, before reporting):**
```
all batch scenarios green → run full suite → regressions?
    → fix regression → re-run → repeat (max 3)
    → run QA gates → failures?
        → fix → re-run → repeat (max 3)
    → gap-finding → plan revision needed?
        → revise PLAN.md
    → report to human
```

The inner loop is fast and cheap — it's the agent reading error messages and fixing code. The outer loop is thorough and bounded — it's the quality assurance pass that prevents premature batch completion.

## What NOT to Adopt

**Reflexion-style verbal memory** — The agent writing lessons about what went wrong sounds appealing but has a documented failure mode: confabulation. The agent writes a plausible but incorrect diagnosis, then reinforces it on subsequent attempts. Craftsman Dev's ADR system is the correct alternative: failure lessons are *human-gated*, not agent-generated. The ADR captures what was tried and what failed, but the human validates the diagnosis.

**RALPH-style planless iteration** — Pure gap-finding without a plan works on benchmarks but is expensive in practice. Each session re-discovers the next gap from scratch. Craftsman Dev's batched plan is the right middle ground: structured enough to prevent rediscovery, dynamic enough to adapt.

**Unbounded improvement loops** — "Keep improving until perfect" is a recipe for token burn. Every loop needs a budget (3 attempts for implementation, 1 for environment) and an escalation path (ADR + human). The agent's job is not to solve unsolvable problems — it's to identify them and report.

**Agentic stopping decisions** — Zenith lets the orchestrator decide when to stop. Craftsman Dev correctly makes stopping a *mechanical* decision: all scenarios green + all gates pass = done. The agent doesn't get to decide it's done. The test runner does.

**Self-harness evolution** — Academic research on agents that rewrite their own harnesses is fascinating but premature for production. The craftsman decides how the process works. The agent executes within it.

## Comparison with Existing Approaches

| Dimension | Superpowers | RALPH/Zenith | Reflexion | Craftsman Dev (proposed) |
|---|---|---|---|---|
| Plan type | Static monolith | None / milestone-based | N/A (within-task) | Dynamic batched |
| Revision trigger | Never (plan is fixed) | Every session (gap-finding) | Every failed attempt | Batch boundary + gap-finding |
| Recovery mechanism | Subagent retry (unbounded) | Re-open from scratch | Verbal self-critique | Failure-classified, budgeted |
| Stopping condition | Agent declares done | Budget/iteration limit | Verifier passes | All scenarios green + QA gates |
| Failure memory | None (session-scoped) | Project state on disk | Episodic verbal buffer | ADRs (human-gated) |
| Quality signal | Agent review (opinion) | Task validator | External evaluator | Mechanical (lint + health + test) |
| Human involvement | Brainstorming only | None during execution | None during execution | Batch boundary gate + on-demand review |

## Conclusion

Craftsman Dev's adaptive planning architecture is structurally sound. The batched plan, dynamic revision, and three-attempt stuck protocol align with the 2026 research on what works. The enhancements are mechanical, not conceptual:

1. **Gap-finding at batch boundaries** — re-read SPEC.md and verify remaining plan coverage before reporting
2. **Failure-classified recovery** — different failure types get different budgets and different recovery actions
3. **QA gates in the improvement loop** — lint, type-check, and health score run automatically at batch boundaries, with bounded fix-and-retry
4. **Observation-grounded replanning** — plan revision uses execution results, not just completion status

The most important finding from the research: **a static plan is worse than no plan at all.** Plan-RALPH underperformed planless RALPH because the plan locked in early assumptions. Craftsman Dev avoids this by making PLAN.md explicitly dynamic — but the skill should make the revision mechanics equally explicit. The plan isn't just "editable." It's *actively questioned* at every boundary.

And the deepest validation: Craftsman Dev's SPEC.md solves the premature completion problem at the architecture level. The agent cannot redefine "done" because the human already defined it mechanically. Every other system in the comparison relies on the agent or a second agent to decide when work is complete. Craftsman Dev's test runner decides. That's the difference between an opinion and a fact.
