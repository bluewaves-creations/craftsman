# Craftsman Dev

> An opinionated software development methodology for agentic coding — born from first principles, not from patching someone else's framework.

---

## Origin

This methodology emerged from a simple question: are frameworks like Superpowers — with their 246K GitHub stars and elaborate ceremony — actually the right architecture for a disciplined developer? The answer, after thorough research, is no. Not because Superpowers is badly built, but because it solves the wrong problem for someone who already knows how to build software.

Superpowers forces brainstorming before every session, dispatches disposable subagents per micro-task (each paying ~14K tokens of system prompt overhead), uses LLM-powered review agents to decide whether code works, and burns $6–15 per modest feature in API costs. Its author, Jesse Vincent, openly acknowledges that token cost is "the most common lament" from users. The framework exists to impose discipline on developers who would otherwise let agents vibe-code without thinking.

Craftsman Dev takes a different stance. If you already have discipline, you don't need a framework to simulate it. What you need is a clean separation of responsibilities, mechanical verification you can trust, and persistent artifacts that accumulate project intelligence across sessions without burning tokens to re-derive it every time.

## The Three-Actor Model

Every responsibility in the development process belongs to exactly one actor. No overlap, no confusion, no redundancy.

| Actor | Strength | Responsibility |
|---|---|---|
| Human | Vision, taste, judgment | AGENTS.md, intent, quality gates, when to review |
| Agent | Literacy, synthesis, code | SPEC.md, PLAN.md, implementation, step definitions |
| Machine | Determinism | Verification — exit codes, screenshots, DOM assertions |

Three rules govern the boundaries:

1. **Never use an agent for verification.** An LLM can hallucinate "PASS" just as easily as it can hallucinate working code. Superpowers' two-stage review subagents — spec compliance and code quality — are LLM opinions about LLM output. Turtles all the way down. Craftsman Dev runs the tests and reads the exit code. Zero tokens, incorruptible.

2. **Never use a machine for design.** Acceptance criteria, architectural decisions, and quality standards are human responsibilities. The machine doesn't judge whether code is good. It judges whether code satisfies a mechanically verifiable assertion.

3. **Never make the human do what the agent does well.** The agent reads documentation, translates requirements into Gherkin scenarios, writes step definitions, writes implementation code, manages the plan. The human provides intent and authoritative sources. The agent is the librarian.

## Agent Is the Librarian

This is the foundational pattern, consistent across every system built under this philosophy — not just software development.

The agent reads official documentation provided by the human (links or MCPs), synthesizes domain knowledge, and produces faithful artifacts. Gherkin scenarios, step definitions, implementation code — all translated from authoritative sources, never invented from training data.

The agent never guesses. If documentation is missing, it asks for it. If an API surface is unclear, it reads the docs before writing a single line. This is not a suggestion — it is a non-negotiable constraint. The most common failure mode in agentic development is hallucinating APIs that don't exist or using deprecated patterns from stale training data. The librarian pattern eliminates this by making documentation the starting point of every implementation task, not a fallback.

## Non-Negotiable Constraints

These are the constitution. Everything else follows from them.

- **Production grade. Always.** No shortcuts, no stubs left behind, no "good enough for now." Zero implementation gap tolerance.
- **Official documentation driven.** Never rely on training data for API surfaces, framework behavior, or platform specifics. The human provides documentation links or MCPs. Read them first, code second.
- **No agentic verification.** Never ask an LLM whether code works. Run the tests. Read the exit code.
- **No per-session brainstorming.** AGENTS.md is the brainstorm artifact. Read it, don't re-derive it.
- **No monolith plans.** Batches keep PLAN.md lean and context-friendly.
- **Spec is static.** Only the human changes SPEC.md. The agent changes PLAN.md and code, never acceptance criteria.
- **Compress, don't spawn.** Prefer context compression between batches over subagent dispatch per task.
- **Stuck: three attempts.** If a scenario won't go green after three attempts, stop. Report what was tried. Draft an ADR for the failure. Do not keep retrying.
- **Composable, not monolithic.** On Apple platforms, compose with Xcode 27 skills. On other stacks, compose with stack-specific skills. This skill owns process, not platform idiom.

## Artifact Architecture

Five persistent artifacts hold the project's accumulated intelligence. Nothing lives in agent memory. A fresh agent with access to these five knows everything a returning agent would — with zero warm-up tokens.

```
AGENTS.md (human, static)
    ↓ informs
SPEC.md (agent-drafted, human-approved, Gherkin, static)
    ↓ drives
PLAN.md (agent, batched, dynamic)
    ↓ executes into
git log (structured commits, append-only ledger)
    ↓ feeds learnings into
decisions/ (human-gated, consolidated ADRs)
```

Every artifact has one owner, one lifecycle, one purpose. The information flows one direction: vision → spec → plan → history → knowledge.

### AGENTS.md — The Permanent Context

Static. Human-authored. Loaded once per session.

The project's vision, design guidelines, fundamental architecture, tech stack, and what good looks like. This replaces per-session brainstorming entirely. Superpowers burns ~20K tokens every session re-discovering what the developer wants. AGENTS.md costs ~200 tokens on cache reads after turn 1 — a 100× reduction for the same function.

If AGENTS.md doesn't exist yet, the agent helps the human write one by interviewing for purpose, users, core abstractions, tech choices, and quality bar. Keep it under 3K tokens — it loads every session.

### SPEC.md — The Executable Specification

Static. Gherkin features. Drafted by the agent as librarian — reading official documentation provided by the human — then approved by the human.

Each feature is a Gherkin scenario:

```gherkin
Feature: Todo management
  Scenario: Add a todo item
    Given a user is logged in
    When they add "Buy milk"
    Then the todo list contains "Buy milk"

  Scenario: Complete a todo item
    Given a todo "Buy milk" exists
    When the user marks "Buy milk" as done
    Then "Buy milk" appears in the completed list
```

SPEC.md is the single source of truth for "what done looks like." It does not change during implementation unless the human changes requirements. Every scenario starts red. Implementation is done when all are green.

This is the core architectural decision that separates Craftsman Dev from Superpowers: the specification *is* the test suite. A Gherkin scenario is simultaneously a human-readable requirement, an acceptance criterion, and an executable verification. The agent can't game it. Either the test runner goes green or it doesn't. No opinion involved.

### PLAN.md — The Dynamic Roadmap

Dynamic. Agent-maintained. Adapts after each batch based on what was learned.

Batches of tasks that turn red scenarios green. Each batch targets specific scenarios and has clear, mechanically verifiable success criteria:

```markdown
## Batch 1: Core data model + first scenario
Target scenarios: Add a todo item
Tasks:
- Set up project scaffold and test harness
- Implement Todo model
- Implement add endpoint
- Wire step definitions
Success: `craftsman verify --batch 1` exits 0

## Batch 2: Completion flow
Target scenarios: Complete a todo item
Tasks:
- Add completion state to model
- Implement completion endpoint
- Wire step definitions
Success: `craftsman verify --batch 2` exits 0
```

The spec doesn't move; the plan does. If batch 1 reveals that a different approach is needed for batch 3, the plan updates. The acceptance criteria don't.

Batching prevents the monolith-plan problem documented in Superpowers' own GitHub issue #512: a 48K-character plan gets re-read 3–4 times as context compresses, costing 45–60K tokens. Batched plans keep each scope small enough to execute without re-reading.

### Git Log — The Chronological Ledger

Append-only. Mechanical. Zero tokens at rest, queryable on demand.

Structured commit messages make the project's history self-documenting:

```
feat(batch-1): Add a todo item → GREEN

Implemented: Todo model, add endpoint, step definitions
Scenarios: "Add a todo item" passing
Learned: In-memory store sufficient for MVP
Ref: PLAN.md batch 1, SPEC.md lines 3-7
```

The agent can `git log` to understand what happened, in what order, and why — without keeping any of it in context. This replaces any need for session-spanning memory or conversation history.

### decisions/ — Architecture Decision Records

Append-only by default. Human-gated consolidation at the finish step.

ADRs prevent the most expensive failure mode in agentic development: the agent confidently re-attempting something that was already tried and rejected. Without a ledger, every new session starts naive.

```
decisions/
  active/
    data-architecture.md      ← consolidated from ADR-001, 003, 007
    auth-strategy.md           ← consolidated from ADR-004, 005
    ui-framework.md            ← single decision, never needed merging
  index.md                     ← one-liner per active decision, <500 tokens
```

The agent reads `index.md` first — under 500 tokens to know what's been decided. Only opens a full decision file if current work touches that domain. Same pattern as a skills catalog: lean index, on-demand depth.

**ADR lifecycle:**

- **Record** — individual decisions written as they happen. Low ceremony.
- **Consolidate** — related decisions merge at the finish step. Five ADRs about data layer choices become one `data-architecture.md` with the full trail compressed. The "tried and failed" history stays, but as terse lines.
- **Supersede** — originals absorbed into the consolidated record. Git history preserves full forensics if anyone needs them.

Consolidation is human-gated: the agent proposes merges, the human approves. Without this grooming cycle, fifty ADRs bloat the directory and the agent burns 20K tokens reading what not to do. With it, `decisions/` stays proportional to the project's architecture, not its history.

ADR format:

```markdown
# ADR-003: In-memory store over SQLite for MVP

## Status: Accepted
## Context
Batch 1 implementation revealed SQLite added complexity without
value at this stage.
## Tried
SQLite with migrations — worked but 4x setup overhead for MVP scope.
## Decision
In-memory store. Revisit when persistence batch begins.
## Consequences
No data survives restart. Acceptable for MVP.
```

## Verification Stack

Four layers, each catching a different class of problem, each using the right tool for the job.

| Layer | Question | Mechanism | Actor |
|---|---|---|---|
| Functional | Does it work? | `craftsman verify` / `swift test` | Machine |
| Structural | Is it good? | Code-reviewer agent | Agent (on demand) |
| Visual | Does it look right? | Playwright / device-interaction | Machine |
| Historical | Have we tried this before? | `decisions/index.md` | Agent (on demand read) |

### Functional Verification

Mechanical. Exit 0 means green. Anything else means red. No LLM interprets the result.

**Python / TypeScript / Rust** — a unified CLI abstracts Gherkin execution across stacks:

```bash
craftsman verify              # run all scenarios
craftsman verify --batch 2    # run batch 2 scenarios only
craftsman verify --scenario "Add a todo item"  # run one
```

The CLI maps Gherkin steps to the stack's native test runner and returns structured results. The agent reads the exit code and stdout — never judges pass/fail itself.

**Swift** — Swift Testing (the recommended framework since Xcode 27) with Gherkin scenarios mapped to parameterized `@Test` functions using `#expect`. Composable with Apple's bundled Xcode 27 skills:

- **test-modernizer** — XCTest → Swift Testing migration
- **swiftui-specialist** — SwiftUI conventions and best practices

### Structural Review (Code Quality)

On demand. The human knows which work deserves scrutiny — not every batch, not on a schedule.

A dedicated permanent agent reviews architecture, clean code, naming, patterns, and unnecessary complexity. This is the one place where agentic opinion is appropriate: "is this code good?" is a judgment call. "Does this code work?" is not — that's the test runner's job.

The reviewer agent loads AGENTS.md as its quality bar. It doesn't invent standards — it applies the ones the human defined.

### Visual Verification

Mechanical. Screenshots, DOM/hierarchy assertions, layout comparison.

**Web:** Playwright — screenshot comparison, DOM assertions, visual regression testing. Not "does this look right?" asked to an LLM, but pixel-level and structural comparison.

**Apple platforms:** Xcode 27's `device-interaction` skill — simulator screenshots, UI hierarchy inspection, synthesized touch interactions. Verifies layout across screen sizes mechanically. Combined with Device Hub, agents can run a complete visual verification loop: install, navigate, capture, compare.

### Historical Verification

On-demand read. The agent checks `decisions/index.md` before proposing any architectural approach. If a relevant ADR exists, it reads the full record. If the proposed approach was already tried and rejected, the ADR tells it why.

## Workflow

### 1. Bootstrap (once per project)

Check for AGENTS.md. If missing, help the human write one.

Check for SPEC.md. If missing, spawn an expert agent as librarian to draft Gherkin scenarios from requirements and official documentation provided by the human. The human approves the final SPEC.md.

Set up the verification harness matching the tech stack in AGENTS.md. Wire step definitions so that `craftsman verify` (or `swift test` on Apple platforms) can execute scenarios mechanically.

### 2. Plan

Read SPEC.md. Identify all scenarios. Group into batches of 2–4 related scenarios. Write PLAN.md with tasks per batch. Each batch names its target scenarios explicitly.

### 3. Execute (per batch)

Work through the current batch sequentially in the current context. No subagent dispatch for implementation — stay in context, keep continuity.

For each task:

1. Read official documentation for any APIs involved
2. Write or update step definitions for target scenarios
3. Run verification — confirm scenarios are red (if new)
4. Implement the minimum code to pass
5. Run verification — confirm green
6. Refactor if needed, re-confirm green

If a task reveals something that changes the plan, update PLAN.md before moving on.

If a scenario won't go green after three attempts: stop, report what was tried, draft an ADR, wait for the human.

### 4. Batch Boundary

When a batch's target scenarios all pass:

1. **Stop.** Do not start the next batch automatically.
2. Run full verification (all scenarios) to catch regressions.
3. Report: green scenarios, remaining red, lessons learned.
4. Commit with structured message to the git ledger.
5. Compress context if the conversation is long.
6. Optionally refine PLAN.md for remaining batches.
7. Wait for the human to say "next" or redirect.

### 5. Finish

When all scenarios in SPEC.md are green:

1. Full QA pass — all scenarios, lint, type-check.
2. If failures: fix → re-run → repeat (improvement loops).
3. Propose ADR consolidation — human approves merges.
4. Update or generate documentation.
5. Final commit referencing completed scenarios.
6. Branch/PR only if AGENTS.md indicates a production app exists.

## Agent Architecture

### The Librarian Pattern (perpetual)

The primary agent reads documentation, translates domain knowledge into executable form, and implements. It stays in the main context across a batch. It compresses between batches rather than spawning fresh subagents. This preserves continuity and eliminates the ~14K-token system prompt tax that Superpowers pays per subagent launch.

### Research Agents (isolated, on demand)

Spawn a dedicated agent for unfamiliar APIs, library evaluation, or architectural exploration. Keep research token burn isolated from implementation context. The research agent returns findings; the implementation context stays clean.

### Code Review Agent (dedicated, on demand)

A permanent agent for structural review. Invoked when the human decides, not on a schedule. Loads AGENTS.md as its quality bar. Reviews architecture, patterns, naming, complexity. The one place where agentic opinion earns its keep.

### Visual Verification (platform-specific)

**Web:** Playwright agent — mechanical screenshots and assertions.
**Apple:** Xcode 27 `device-interaction` — mechanical screenshots, hierarchy inspection, synthesized touch.

Both are invoked as needed for visual work. Neither asks an LLM for aesthetic judgment.

## Comparison with Superpowers

| Dimension | Superpowers | Craftsman Dev |
|---|---|---|
| Vision & design | Agent brainstorms every session (~20K tokens) | Human writes AGENTS.md once (~200 tokens cached) |
| Specification | Agent-generated, agent-reviewed | Agent drafts from docs, human approves, mechanically executable |
| Verification | Review subagent (LLM opinion) | Test runner (exit code) |
| Planning | Agent writes monolith plan | Dynamic PLAN.md, batch-scoped |
| Implementation | Disposable subagent per micro-task | Continuous context, compress between batches |
| Code review | Automated two-stage subagent review | On-demand dedicated agent, human-triggered |
| Visual testing | Not addressed | Playwright (web), device-interaction (Apple) |
| Knowledge preservation | Session-scoped, lost on restart | Git ledger + consolidated ADRs |
| Stuck handling | Subagent retries indefinitely | Three attempts, then ADR and stop |
| Documentation dependency | Training data | Official docs provided by human |
| Typical cost per feature | $6–15, hours of wall-clock | Fraction — no subagent overhead, no brainstorming ceremony |

## Token Economics

The economic argument is structural, not incremental.

Superpowers' per-task loop: implementer subagent (~14K system prompt overhead) + reviewer subagent (~14K overhead) + coordinator orchestration = ~30K+ tokens of agent-verifying-agent ceremony per task. For 9 tasks, that's 27+ subagent launches.

Craftsman Dev's per-task loop: read docs → write code → `craftsman verify` returns exit code. The verification step costs zero tokens. The implementation stays in continuous context with no subagent system prompt tax. Compression between batches reclaims space without losing narrative continuity.

The brainstorming delta alone: Superpowers re-derives project context every session (~20K tokens). AGENTS.md loads once (~3K tokens raw, ~300 tokens on cache reads after turn 1). Over a 20-session project, that's ~400K tokens saved on brainstorming alone.

## Design Philosophy

Craftsman Dev does not exist to impose discipline on undisciplined developers. It exists to give a disciplined developer the minimal, honest infrastructure needed to work effectively with agentic tools.

It assigns each actor to what they do best. Humans bring vision and taste. Agents bring literacy and synthesis. Machines bring truth. The methodology's job is to keep those boundaries clean and make sure no actor is asked to do what another does better.

The result is not a framework. It is a practice — lean enough to internalize, opinionated enough to prevent drift, and composable enough to work with whatever platform skills the ecosystem provides.
