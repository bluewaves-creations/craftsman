# Implementation Quality: Research & Alternatives

> Best practices, conventions, documentation-first enforcement, and clean code guidelines for systematic application in agentic development — evaluated against what Craftsman Dev already describes.

---

## The Question

Craftsman Dev currently says "official documentation driven, never rely on training data" and "production grade, always." But *how* do you enforce that systematically? What tools, patterns, and mechanisms exist in 2026 to make documentation-first coding and clean code standards mechanical rather than aspirational?

## What We Already Have

Craftsman Dev's current constraints:

1. **Official documentation driven** — human provides links or MCPs, agent reads before coding
2. **Production grade, always** — zero implementation gap tolerance
3. **Agent is the librarian** — reads docs, never invents
4. **AGENTS.md** — project-level vision, architecture, tech stack

The question is whether these constraints, as stated, are sufficient — or whether the 2026 landscape offers mechanisms to make them harder to violate.

## The Three Layers of Quality Enforcement

Research from CodeScene, Anthropic, Stack Overflow, and academic papers converges on a three-layer model for agentic code quality. Each layer uses a different enforcement mechanism:

### Layer 1: Deterministic Tools (Machines — Already Solved)

Linters, formatters, type checkers, static analyzers. These are the *wrong* things to put in AGENTS.md.

A 2026 paper on "Configuration Smells in AGENTS.md Files" found that the most common anti-pattern is duplicating rules that linters already enforce: naming conventions, import ordering, indentation, maximum line length. This wastes context tokens and diverts the agent from architectural and domain concerns that actually need human guidance.

The principle: **if a deterministic tool catches it, don't restate it.** Configure ESLint/Prettier/SwiftLint/Ruff/clippy once. Let CI enforce it. The agent runs `lint --fix` as part of its workflow. This is already implicit in Craftsman Dev but should be explicit.

### Layer 2: Documentation Grounding (Agents + Machines)

This is the layer where the 2026 landscape has genuinely new tools.

**The Problem:** LLMs hallucinate APIs. They generate code using deprecated patterns, non-existent methods, and fantasy parameters. A developer survey quote captures it precisely: "I never again want to give two shits about the specific best way to quijibo the toaster in dingledangle framework v0.21. The agent reads the latest docs and then is forced to comply."

**Context7 MCP** (by Upstash, open-source, MIT) is the most mature solution. It's an MCP server that fetches up-to-date, version-specific library documentation directly into the agent's context. The agent queries Context7 before generating code, gets current API surfaces and examples, and produces code grounded in reality rather than training data.

How it works: the agent resolves a library name → gets version-specific docs → generates code from current examples. The prompt includes `use context7` or references a library ID like `/supabase/supabase`. Context7 indexes public documentation continuously as libraries release new versions.

For Craftsman Dev, Context7 could *mechanize* the "official documentation driven" constraint. Instead of relying on the human to provide links every time, Context7 provides a standing pipeline of current docs. The human still provides docs for proprietary or domain-specific APIs; Context7 handles the open-source library layer.

**Docs MCP Server** (open-source alternative to Context7) goes further — it scrapes and indexes any documentation source (websites, GitHub, npm, PyPI, local files, PDFs, even Office documents) into a personal documentation corpus. The agent queries this corpus before coding. This is the self-hosted, private version of the same pattern.

**GROUNDING.md** is an academic concept (arXiv 2604.21744, April 2026) that formalizes documentation-first coding at the domain level. It separates two categories:

- **Hard Constraints (HCs):** Non-negotiable validity invariants that override all other context, including user prompts. Example from proteomics: "FDR must be ≤ 0.01 via target-decoy approach." The agent cannot violate this even if asked to.
- **Convention Parameters (CPs):** Community-agreed defaults that can be overridden with justification. Example: "Default mass tolerance: 10 ppm."

The GROUNDING.md concept is domain-specific (the paper uses mass spectrometry), but the architecture generalizes. For software development, hard constraints might be: "Never use `any` type in TypeScript," "All Swift concurrency must use structured concurrency (`async`/`await`), never GCD," "All API endpoints must validate input before processing." Convention parameters might be: "Default error response format: RFC 7807 Problem Details," "Logging: structured JSON with correlation ID."

For Craftsman Dev, GROUNDING.md maps directly to what AGENTS.md already does — but with a sharper distinction between what's non-negotiable and what's conventional. This distinction matters for the agent: a hard constraint means "refuse and ask," a convention means "use unless told otherwise."

### Layer 3: Structural Quality (Agents — On Demand)

This is the code review layer. CodeScene's research provides the strongest evidence:

- **60% higher defect risk** when AI works on unhealthy code (peer-reviewed, 2026)
- **50% more tokens burned** on unhealthy code
- **Code Health threshold of 9.5** needed for reliable AI performance (industry average is 5.15)

**CodeScene CodeHealth MCP Server** measures code quality mechanically — 25+ factors including complexity, cognitive load, coupling, and maintainability — and returns a score from 1 to 10. The agent runs `code_health_review` on its own output, gets specific maintainability issues, and can refactor before committing.

This is the mechanical version of Craftsman Dev's code review agent. Not a replacement — the human-triggered code review still handles architectural judgment. But CodeHealth can run automatically at batch boundaries as a quality gate: "If any file scores below 8.0, refactor before proceeding."

## What Craftsman Dev Should Adopt

### 1. Tiered Convention Architecture in AGENTS.md

Replace the flat "what good looks like" section with three tiers:

**Hard Constraints** — non-negotiable, refuse-and-ask if violated:
```markdown
## Hard Constraints
- All API surfaces must be verified against official documentation before implementation
- No `any` types in TypeScript, no force-unwraps in Swift
- All concurrency: structured (async/await), never callbacks or GCD
- All errors: typed, never string-only
- All external input: validated at the boundary
```

**Conventions** — defaults, overridable with explicit justification:
```markdown
## Conventions
- Error responses: RFC 7807 Problem Details format
- Logging: structured JSON, correlation ID, ISO 8601 timestamps
- Naming: domain terms from AGENTS.md glossary
- File organization: feature-based modules, not layer-based
```

**Style** — deferred entirely to deterministic tooling:
```markdown
## Style
Enforced by tooling. Do not duplicate here.
- Python: ruff (format + lint)
- TypeScript: biome
- Swift: swift-format + SwiftLint
- Rust: rustfmt + clippy
Run `craftsman lint` before committing.
```

This maps to the GROUNDING.md architecture (HC/CP) but adapted for software craftsmanship. The critical insight from the AGENTS.md research: **one code example per convention beats three paragraphs describing it.** Show, don't tell. LLMs know how to write Python — they don't know *your* Python.

### 2. Documentation Pipeline (Context7 or Equivalent)

Mechanize the "official documentation driven" constraint:

**For open-source libraries:** Connect a Context7 MCP (or Docs MCP Server) that the agent queries automatically before implementation. The agent never writes code against an unfamiliar library without first fetching current docs.

**For proprietary/domain APIs:** The human provides documentation links or local doc files. The agent reads them before coding. This is already in the workflow but could be made more explicit: step 1 of every implementation task is "read documentation for any APIs involved."

**For Apple platforms:** Xcode 27's skills and Apple's documentation are the primary source. Compose with Apple's bundled skills, which already encode platform-specific conventions.

The pipeline: human provides doc source → agent fetches current docs → agent generates code grounded in those docs → mechanical verification confirms behavior. No step involves the agent relying on training data for API surfaces.

### 3. Mechanical Quality Gates

Add deterministic quality checks at batch boundaries, alongside the Gherkin verification:

```bash
craftsman verify --batch 2     # Gherkin scenarios pass? (functional)
craftsman lint                 # Linter/formatter clean? (style)
craftsman health               # CodeHealth ≥ 8.0? (structural)
```

If any gate fails, the batch isn't done. The agent fixes issues before reporting to the human. This is entirely mechanical — no agentic opinion involved. The code review agent remains on-demand for architectural judgment that tools can't measure.

### 4. The "Show Don't Tell" Principle for AGENTS.md

The strongest finding from the 2026 AGENTS.md research: **concrete code examples are the highest-leverage content.** Abstract rules ("use clean code practices") waste tokens. Concrete examples ("error handling looks like THIS") ground the agent.

```markdown
## Error Handling Convention
```python
# Good
class OrderNotFoundError(DomainError):
    def __init__(self, order_id: str):
        super().__init__(f"Order {order_id} not found", code="ORDER_NOT_FOUND")

# Bad — never do this
raise Exception("order not found")
```

One example per convention. The agent pattern-matches on examples far more reliably than it follows abstract prose.

### 5. AGENTS.md Hygiene (agents-lint)

A 2026 ETH Zurich study found that stale or inaccurate context files reduce task success by 2-3% while increasing cost by 20%. **agents-lint** is a zero-dependency CLI that validates AGENTS.md against the actual repo: checking that referenced paths exist, npm scripts are real, framework patterns are current.

For Craftsman Dev, this is a natural addition to the finish step: after all scenarios pass, before committing, run `agents-lint` to verify that AGENTS.md still reflects reality. If the implementation changed the project structure, AGENTS.md should be updated.

## What NOT to Adopt

**Kiro's Hooks** — Event-driven automations ("on file save, regenerate tests") are powerful for team environments but add complexity for a solo craftsman. The batch-boundary workflow already handles the same concerns: lint, verify, health-check all happen at the boundary, not on every save. Hooks solve the "developer forgets to run tests" problem. A craftsman doesn't forget.

**BMAD's 12-agent personas** — Analyst, PM, Architect, Developer, UX, QA, each with their own prompt. This is organizational theater for solo development. The craftsman is the architect, the PM, and the quality gate. The agents are librarians and reviewers, not role-players.

**AI-driven quality assessment as verification** — Some tools (ContextQA, testRigor) use LLMs to evaluate whether code meets spec. This is exactly the failure mode Craftsman Dev rejects. Quality assessment by LLM is opinion. Quality assessment by linter, type checker, and CodeHealth is measurement.

## Comparison: What Each Approach Covers

| Concern | Craftsman Dev (current) | + Recommended | Kiro | Spec Kit | BMAD |
|---|---|---|---|---|---|
| Doc grounding | Human provides links | Context7 MCP | Steering files | None | None |
| Style enforcement | Implicit | Explicit tooling tier | Hooks | None | Agent persona |
| Hard constraints | In AGENTS.md (flat) | HC/CP tiering | Steering files | Constitution | PRD |
| Code quality | Review agent (on demand) | + CodeHealth gate | Hooks | None | QA agent |
| Convention examples | Not explicit | Show-don't-tell | Steering examples | None | Story templates |
| Context hygiene | Not addressed | agents-lint | Manual | Manual | Manual |

## Conclusion

Craftsman Dev's current constraints are directionally correct. "Official documentation driven" and "production grade, always" are the right principles. What the 2026 landscape adds is **mechanisms to make these principles harder to violate:**

1. **Tier the constraints** (hard / convention / style) so the agent knows what's non-negotiable vs. defaultable vs. tool-enforced
2. **Mechanize doc grounding** via Context7 or equivalent so "read the docs first" isn't just an instruction but an automated pipeline
3. **Add mechanical quality gates** (lint + CodeHealth) at batch boundaries so structural quality is measured, not hoped for
4. **Show, don't tell** — concrete code examples in AGENTS.md outperform abstract rules by a wide margin
5. **Keep AGENTS.md accurate** — validate against the repo at the finish step

None of these require new conceptual machinery. They're mechanical implementations of principles already in the skill. The architecture holds. It just needs sharper teeth.
