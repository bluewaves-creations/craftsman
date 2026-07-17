# The Ledger Architecture: Research & Alternatives

> ADRs and git commits as complementary ledgers for project intelligence — evaluated against the 2026 landscape of agent memory, event-sourced knowledge, and structured commit protocols.

---

## The Question

Craftsman Dev uses two ledgers: **git commits** (chronological record of what happened) and **ADRs** (architectural decisions with consolidated grooming). Are these the right two? Are they sufficient? Does the 2026 research on agent memory offer something better?

## Why Two Ledgers

The intuition behind two ledgers is sound and the research validates it. Each ledger answers a different class of question:

| Question | Git Log | ADRs |
|---|---|---|
| What happened? | ✓ Chronological, immutable | |
| When did it happen? | ✓ Timestamps | |
| Why was this decided? | | ✓ Context, alternatives, consequences |
| What was tried and failed? | | ✓ "Tried" section |
| What's the current architecture? | | ✓ Active decisions (consolidated) |
| What changed in this file? | ✓ git diff, git blame | |

Git captures the *narrative*. ADRs capture the *reasoning*. Neither substitutes for the other — a commit message tells you the data model changed; the ADR tells you why PostgreSQL was chosen over DynamoDB, what was tried first, and when to revisit.

## The 2026 Landscape

Three significant developments challenge or enrich the two-ledger model:

### 1. The Decision Shadow (Lore Protocol)

A March 2026 paper introduces the concept of the **Decision Shadow**: "Every commit captures a code diff but discards the reasoning behind it — the constraints, rejected alternatives, and forward-looking context that shaped the decision."

This reasoning gap is precisely what ADRs exist to fill. But ADRs have a synchronization problem: as separate files, they must be manually maintained alongside an evolving codebase. The Lore protocol proposes a radical alternative: **restructure the commit message itself into a self-contained decision record** using native git trailers.

A Lore-enriched commit:

```
feat(batch-1): implement in-memory todo store

Implement Todo model with add/complete operations.
Scenarios: "Add a todo item" now passing.

Constraint: MVP scope — no persistence requirement yet
Rejected: SQLite with migrations — 4x setup overhead for MVP
Learned: In-memory store sufficient; revisit at persistence batch
Ref: SPEC.md lines 3-7, PLAN.md batch 1
Verified-by: craftsman verify --batch 1
Supersedes: none
```

The trailers (`Constraint:`, `Rejected:`, `Learned:`, `Ref:`, `Verified-by:`) are native git metadata — queryable via `git log --format`, grep-able, diff-able, and discoverable by any agent capable of running shell commands. No infrastructure beyond git.

Lore's argument against separate ADRs: ADR files are a synchronization burden that drifts from the code they describe. By embedding the decision metadata in the commit that implements the decision, the knowledge is physically co-located with the change and immutable by construction.

**Assessment for Craftsman Dev:** Lore is compelling for *implementation decisions* — the tactical choices made within a batch. But it doesn't replace ADRs for *architectural decisions* that span multiple commits, multiple batches, or the entire project. "We chose PostgreSQL" isn't a single commit's decision. "We used an in-memory store for batch 1" is. The two are complementary, not competing.

### 2. Event-Sourced Memory (projectmem)

A June 2026 academic paper introduces **projectmem**: an append-only, plain-text event log of typed events — issues, attempts, fixes, decisions, and notes — that is deterministically projected into compact, AI-readable summaries.

The key innovation: **Memory-as-Governance**. projectmem doesn't just store history — it adds a deterministic pre-action gate that warns an agent *before* it repeats a previously failed fix or edits a known-fragile file. Memory becomes active rather than passive.

Event types in projectmem:

```
ISSUE:   "Auth middleware fails with expired JWT tokens"
ATTEMPT: "Tried refreshing token in middleware — caused infinite loop"
FIX:     "Moved token refresh to interceptor layer"
DECISION: "Use interceptor pattern for all token operations"
NOTE:    "JWT library v3 deprecated; v4 migration needed"
```

The log is append-only, grep-able, diff-able, and git-native — no vector database, no embeddings. A deterministic projection compiles the raw log into a compact summary the agent reads at session start, consuming far fewer tokens than re-reading the full history.

Stale-memory detection cross-checks every decision that cites a file against that file's git history. When the file has moved on, the memory is flagged ("predates 7 commits to auth.py — confirm or supersede"). The human decides whether to confirm or replace.

**Assessment for Craftsman Dev:** projectmem's event types map almost exactly to what Craftsman Dev already captures across its two ledgers:

| projectmem event | Craftsman Dev equivalent |
|---|---|
| ISSUE | Gherkin scenario (red) |
| ATTEMPT | Improvement loop iteration |
| FIX | Implementation that turns scenario green |
| DECISION | ADR |
| NOTE | Git commit body / ADR context |

The value-add is the **pre-action gate** — a mechanical check that prevents the agent from repeating a previously failed approach. This is something Craftsman Dev's ADRs provide passively (the agent reads `decisions/index.md` before proposing an approach) but projectmem provides actively (the gate fires before the action, not after the agent reads the history).

### 3. CommitDistill (Knowledge Extraction from Git)

A May 2026 paper presents CommitDistill: a tool that mines git history into typed knowledge units — **Facts**, **Skills**, and **Patterns** — using deterministic regex heuristics. No LLM, no embeddings, no vector database.

The insight: git commit history already contains enormous amounts of project knowledge — but in unstructured form. CommitDistill extracts it:

- **Facts**: "Payment service uses Stripe API v3" (from commit messages mentioning API versions)
- **Skills**: "To handle rate limiting, use exponential backoff with jitter" (from fix commits that solved rate-limiting issues)
- **Patterns**: "Auth changes are always paired with test updates" (from co-occurrence patterns in commit history)

**Assessment for Craftsman Dev:** This is downstream of the two-ledger architecture. If Craftsman Dev writes high-quality structured commits (Conventional Commits + Lore trailers), a tool like CommitDistill becomes dramatically more effective because the input is already structured. The quality of knowledge extraction depends on the quality of the source material.

### 4. Agent Decision Records (AgDR)

A January 2026 project extends ADR format specifically for AI agent decisions, adding metadata about which agent made the decision, which model was used, and what triggered it:

```yaml
---
id: AgDR-0001
timestamp: 2026-01-30T18:45:00Z
agent: claude-code
model: claude-opus-4-5
trigger: user-prompt
status: executed
---
```

**Assessment for Craftsman Dev:** Partially relevant. In Craftsman Dev, the agent doesn't make architectural decisions — the human does (or approves them). The agent makes implementation decisions within the constraints of AGENTS.md and SPEC.md. Those implementation decisions are better captured in Lore-style commit trailers than in separate AgDR files, because they're physically tied to the code that implements them.

## The Complementarity Thesis

Your intuition is validated by the research, and can be stated precisely:

**Git commits are the tactical ledger.** They record what happened, when, and in what order. With Conventional Commits + Lore trailers, they also capture why this specific change was made, what was rejected at the implementation level, and what was learned.

**ADRs are the strategic ledger.** They record decisions that span multiple commits, shape the project's architecture, and need consolidation over time. They capture why the project is structured the way it is, not why a specific line of code was written.

The two ledgers have fundamentally different lifecycles:

| Property | Git Log | ADRs |
|---|---|---|
| Granularity | Per-change | Per-decision |
| Mutability | Immutable (append-only) | Append-only, but consolidated |
| Lifecycle | Permanent | Record → Consolidate → Supersede |
| Staleness risk | None (tied to code) | High (separate from code) |
| Token cost to read | High (full history) | Low (index.md) |
| Query method | `git log --grep`, `git blame` | Read index.md, open on demand |

The staleness asymmetry is the critical observation. Git commits never go stale because they're immutable snapshots of what actually happened. ADRs can go stale because they're assertions about the current architecture that may no longer be true. This is why Craftsman Dev's consolidation lifecycle (record → consolidate → supersede) is essential — and why projectmem's stale-memory detection is worth stealing.

## Recommended Architecture

### Three-Layer Ledger

Rather than two ledgers, Craftsman Dev should conceptualize three layers of project memory, each with different granularity, lifecycle, and query cost:

**Layer 1: Git Log (tactical, immutable, per-change)**

Structured commits using Conventional Commits + enriched trailers:

```
feat(batch-1): Add a todo item → GREEN

Implement Todo model with in-memory store, add endpoint,
step definitions for "Add a todo item" scenario.

Scope: PLAN.md batch 1
Scenarios: "Add a todo item" passing
Learned: In-memory store sufficient for MVP scope
Rejected: SQLite — 4x setup overhead, unnecessary for MVP
Ref: SPEC.md lines 3-7
Verified-by: craftsman verify --batch 1
```

The trailers are structured metadata the agent can query:
- `git log --grep="Learned:"` — all lessons learned across project history
- `git log --grep="Rejected:"` — all rejected approaches
- `git log --grep="Ref: SPEC.md"` — all commits tied to spec scenarios

This is Lore's insight applied to Craftsman Dev's commit format. Zero additional files. Zero maintenance burden. The knowledge is immutable, co-located with the code, and queryable.

**Layer 2: ADRs (strategic, consolidated, per-decision)**

Active architectural decisions with the existing lifecycle:

```
decisions/
  active/
    data-architecture.md     ← consolidated from ADR-001, 003, 007
    auth-strategy.md
  index.md                   ← one-liner per active decision, <500 tokens
```

The consolidation cycle at the finish step keeps the directory proportional to architecture, not history. Index.md is the entry point — 500 tokens to know what's been decided.

**Layer 3: Stale-Memory Detection (governance, automated)**

A lightweight check at session start and at the finish step:

- Cross-reference active ADRs against git history: has the file or module an ADR describes been significantly changed since the ADR was written?
- If so, flag the ADR as potentially stale: "data-architecture.md predates 12 commits to `src/models/` — confirm or supersede?"
- The human decides. The agent doesn't silently act on stale knowledge.

This is projectmem's stale-memory detection applied to Craftsman Dev's ADR architecture. No new infrastructure — just a check that compares ADR file timestamps against `git log` for the files they reference.

### Commit Convention for Craftsman Dev

Extend Conventional Commits with batch-aware types and structured trailers:

**Types:**
- `feat(batch-N):` — new functionality that makes a scenario green
- `fix(batch-N):` — correction within a batch
- `refactor(batch-N):` — structural improvement without behavior change
- `test(batch-N):` — step definitions, property tests
- `docs:` — documentation updates
- `chore:` — tooling, dependencies, configuration

**Mandatory trailers at batch boundary commits:**
- `Scenarios:` — which scenarios this commit affects
- `Verified-by:` — the exact verification command and result

**Optional trailers (when applicable):**
- `Learned:` — what was discovered during implementation
- `Rejected:` — what was tried and why it didn't work
- `Ref:` — references to SPEC.md, PLAN.md, or ADRs
- `Supersedes:` — if this commit supersedes a previous approach

### Pre-Action Gate

Before the agent proposes an approach for a task, it should:

1. Read `decisions/index.md` (~500 tokens) — what's been decided architecturally
2. Query `git log --grep="Rejected:" -- <relevant files>` — what's been tried and failed in this area
3. If the proposed approach matches a rejected one, warn: "This approach was previously rejected in commit abc123 because [reason]. Proceed anyway?"

This is projectmem's Memory-as-Governance applied without projectmem's infrastructure. Git log + ADR index provide the same function using tools already in the stack.

## What NOT to Adopt

**Vector databases for project memory** (mnem, mem0) — overkill for a craftsman's workflow. The project's knowledge is in plain text (commits + ADRs), queryable with grep and git log. Embeddings add complexity without proportional benefit for a single-developer project.

**Agent Decision Records (AgDR)** — the agent doesn't make autonomous architectural decisions in Craftsman Dev. Implementation decisions belong in commit trailers. Architectural decisions belong in human-gated ADRs. A separate AgDR format adds a third artifact type without covering a gap.

**Full projectmem deployment** — the event-sourced log with 14 MCP tools and 19 CLI commands is more infrastructure than a personal methodology needs. Steal the concepts (pre-action gate, stale-memory detection), skip the infrastructure.

**Automated ADR generation** — some tools generate ADRs from code changes or agent decisions. In Craftsman Dev, ADR generation is deliberate: the agent drafts, the human approves. Automation removes the human judgment that makes ADRs trustworthy.

## Conclusion

Your instinct is correct: ADRs and git commits are complementary ledgers answering different questions at different granularities. The 2026 research validates this and adds three enhancements worth adopting:

1. **Enriched commit messages** (Lore trailers) — capture the "Decision Shadow" (rejected alternatives, lessons learned, verification metadata) in the commit itself, making git log a queryable knowledge base without separate files
2. **Stale-memory detection** — cross-reference ADRs against git history to flag decisions that may no longer reflect reality, triggered at session start and finish
3. **Pre-action gate** — before proposing an approach, query both ledgers for rejected alternatives, preventing the agent from repeating known failures

The architecture is three layers of the same principle: persistent, queryable, human-gated project memory at different granularities. Git commits for tactics. ADRs for strategy. Stale-detection for hygiene. No vector databases. No embeddings. No infrastructure beyond git and markdown.
