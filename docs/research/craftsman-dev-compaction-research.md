# Context Compaction by Extraction: Research & Architecture

> Preserving high-quality project knowledge across context compressions through structured extraction into a progressive-disclosure knowledge base — evaluated against the 2026 landscape of compaction failures and memory architectures.

---

## The Problem

Default compaction is catastrophic. Claude Code's autocompaction reduced 132K tokens to 2.3K — a 98% reduction — discarding the nuanced understanding that took an entire session to build. Developers report: "Claude was deep into debugging a complex issue. It had the stack trace, the narrowed hypothesis, the exact files. Then auto-compact fired and it literally forgot everything." GitHub issues #13112 and #3274 document cases where compaction corruption became permanent.

The failure modes are structural, not incidental. Default summarization loses exact file paths (the agent searches with vague terms afterward), collapses decision reasoning ("team was active" instead of "chose interceptor pattern because middleware caused infinite loop"), drops hard constraints (the HC tier from AGENTS.md), and overwrites previous compaction summaries instead of building on them. After 2-3 compactions, the agent behaves as if the session just started.

## Why "Compress" Is the Wrong Metaphor

Standard compaction treats context as text to be summarized. Craftsman Dev should treat context as **knowledge to be extracted**. The difference is fundamental:

**Summarization** (lossy): "We worked on authentication and made progress on the todo feature."

**Extraction** (structured): writes to disk a typed knowledge artifact that a fresh context can load on demand, losing nothing of operational value.

The academic research confirms this distinction. A February 2026 paper on "Contextual Memory Virtualisation" frames the problem: "The cost of building context is paid repeatedly, and the resulting understanding is never preserved in a reusable form." The solution is not better summarization — it's externalizing knowledge into persistent, structured artifacts before compression fires.

## The Extraction Architecture

### The Principle: Write to Disk, Not to Summary

At every batch boundary (before compression), the agent extracts structured knowledge into persistent files following the same progressive-disclosure pattern as skills:

```
.craftsman/
  session/
    index.md              ← compact index (~500 tokens)
    batch-1.md            ← detailed transcript of batch 1
    batch-2.md            ← detailed transcript of batch 2
    current-state.md      ← what's green, what's red, what's in progress
    learnings.md          ← accumulated lessons, failed approaches
    open-questions.md     ← unresolved issues, ambiguities
```

**index.md** loads at session start and after every compaction — it's the "where was I?" briefing:

```markdown
# Session State

## Current Position
- Batch 2 of 4 complete
- 3 of 7 scenarios green
- Working on: auth middleware (batch 3)

## Key Decisions This Session
- Chose interceptor pattern over middleware for token ops (see batch-2.md)
- In-memory store confirmed sufficient for MVP (see batch-1.md)

## Active Files
- src/auth/interceptor.ts (modified in batch 2)
- src/models/todo.ts (created in batch 1)
- tests/steps/auth.steps.ts (in progress)

## Open Issues
- JWT library v3 deprecated, v4 migration needed (see open-questions.md)

## Read On Demand
- batch-1.md: data model decisions, store implementation
- batch-2.md: auth architecture, interceptor pattern rationale
- learnings.md: failed approaches (SQLite, middleware pattern)
```

### Progressive Disclosure in Practice

The extraction follows the same three-tier pattern as skills:

**Tier 1: Index** (~500 tokens) — always in context. Current position, key decisions, active files, open issues. This is what loads after compaction.

**Tier 2: Batch transcripts** (~1-2K tokens each) — loaded on demand when the agent needs context about a specific batch. Contains decisions made, approaches tried, files modified, scenarios affected.

**Tier 3: Detailed references** — loaded only when specifically needed. Full error traces, rejected approaches with rationale, architectural reasoning.

The agent reads index.md after every compaction. If it needs more context about batch 1, it reads batch-1.md. If it needs the full error trace from a failed approach, it reads the relevant section of learnings.md. No knowledge is lost — it's just organized by access frequency.

### Surviving Multiple Compactions

The critical failure of default compaction: each compression overwrites the previous summary. After 3 compactions, the agent has a summary of a summary of a summary.

Extraction survives unlimited compactions because the knowledge lives on disk, not in context. After compaction #1, the agent reads index.md and knows exactly where it was. After compaction #5, the same index.md gives the same answer. The extracted files don't compress — they persist.

The accumulation pattern:

```
Batch 1 complete → extract batch-1.md, update index.md
  [compaction fires — no loss, index.md reloads]
Batch 2 complete → extract batch-2.md, update index.md
  [compaction fires — no loss, index.md reloads]
Batch 3 complete → extract batch-3.md, update index.md
  [compaction fires — no loss, index.md reloads]
```

Each extraction is additive. Each compaction is survivable. The agent never starts from scratch.

### What Gets Extracted (and What Doesn't)

**Extract (durable knowledge):**
- Decisions made and their rationale
- Failed approaches and why they failed
- Files created, modified, or deleted
- Scenarios that changed state (red → green or green → red)
- Unresolved questions or ambiguities
- Lessons learned about the codebase

**Don't extract (ephemeral context):**
- Tool call outputs (re-queryable)
- File contents (re-readable from disk)
- Error messages (re-reproducible)
- Intermediate reasoning (derivable from decisions)
- Search results (re-searchable)

The test: if this information disappeared, could the agent reconstruct it by reading files and running commands? If yes, don't extract — it's cheaper to re-derive. If no, extract — it's knowledge that only exists in the conversation.

### Integration with Existing Ledgers

The extraction layer complements (doesn't replace) the two existing ledgers:

| Artifact | Scope | Lifecycle | Survives Compaction? |
|---|---|---|---|
| Git commits | Per-change, immutable | Permanent | Yes (on disk) |
| ADRs | Per-decision, consolidated | Permanent | Yes (on disk) |
| Session extracts | Per-batch, per-session | Session duration | Yes (on disk) |
| Context window | Per-turn | Until compaction | No |

Session extracts are the bridge between the ephemeral context window and the permanent ledgers. At the finish step, relevant session knowledge graduates into permanent artifacts: lessons become commit trailers or ADRs, file lists become commit diffs, decisions become ADR records.

### Relationship to Claude Code Compact Instructions

Claude Code already supports "Compact Instructions" — a section in CLAUDE.md that guides what the summarizer preserves (e.g., `/compact focus on the API changes`). Research shows this improves compaction quality by 49%.

Extraction by disk is complementary: Compact Instructions tell the summarizer what to keep in the reduced context. Extraction tells the agent what to write to disk *before* the summarizer fires. Both can operate together — the summary preserves the gist, the extracted files preserve the details.

### Relationship to projectmem

projectmem's event-sourced log is architecturally similar but more granular: it records every issue, attempt, fix, decision, and note as typed events. Craftsman Dev's extraction is coarser — batch-level transcripts rather than event-level logs. The tradeoff: lower write overhead, slightly less granular retrieval. For a craftsman workflow with human-gated batch boundaries, batch-level extraction is sufficient.

## The Extraction Protocol

Added to the batch boundary workflow:

```
Batch N scenarios all green
    │
    ├── 1. Run full verification
    ├── 2. Run QA gates
    ├── 3. Gap-finding pass
    ├── 4. Commit with structured message
    │
    ├── 5. EXTRACT session knowledge
    │   ├── Write/update .craftsman/session/batch-N.md
    │   │   (decisions, files modified, scenarios affected, learnings)
    │   ├── Update .craftsman/session/index.md
    │   │   (current position, key decisions, open issues)
    │   └── Append to .craftsman/session/learnings.md
    │       (failed approaches, surprises, gotchas)
    │
    ├── 6. Compress context (safe now — knowledge is on disk)
    │
    └── 7. Report to human
```

The extraction happens *before* compression. By the time compaction fires, everything of value is already on disk. The compressed context just needs to be good enough to read index.md and continue — not good enough to remember every detail.

## Conclusion

Compaction by extraction inverts the default: instead of asking "what can the summarizer keep in the shrunk context?" ask "what can the agent write to disk before the context shrinks?" The answer is: everything that matters. Decisions, failed approaches, file lists, scenario states, open questions.

The progressive-disclosure pattern ensures the extracted knowledge doesn't bloat the next session's context. Index.md is ~500 tokens. Batch transcripts load on demand. Detailed references load only when needed. The agent starts each post-compaction turn with the same orientation a fresh human would get from a well-structured handoff document — not a lossy summary of a lossy summary.
