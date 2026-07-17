# Specification Layer: Research & Alternatives

> Thorough analysis of the 2026 landscape for goals, vision, and BDD specification — evaluating whether Gherkin is still the right choice for Craftsman Dev, and what alternatives or complements exist.

---

## The Question

Craftsman Dev currently uses three artifacts for the first part of the flow: AGENTS.md (vision, design, tech stack), SPEC.md (Gherkin scenarios as executable acceptance criteria), and PLAN.md (batched tasks). Before committing to this architecture, we need to know: is Gherkin the best specification language for an agentic, documentation-driven workflow in 2026? Are there more modern approaches? What has the spec-driven development wave produced that's worth stealing?

## The 2026 Landscape

Spec-driven development (SDD) exploded between mid-2025 and early 2026. The trigger was a specific and well-documented failure mode: agents that produce plausible code that drifts from intent, hallucinates APIs, and decays as projects scale. The response was universal — make the specification, not the code or the chat history, the source of truth.

By mid-2026, every major player has shipped their own flavor:

**GitHub Spec Kit** (September 2025, open-source, MIT) — the reference implementation. A CLI (`specify`) plus prompts and slash commands that work across 30+ AI coding tools. Model-agnostic. Four-phase loop: Specify → Plan → Tasks → Implement, each producing a markdown file the next phase reads. Strength: portability across agents. Weakness: rigid phase gates, heavyweight.

**AWS Kiro** (July 2025, GA March 2026) — Amazon's agentic IDE built on Code OSS. Three-phase spec workflow generating requirements.md, design.md, and tasks.md. Also introduces *Agent Hooks* (event-driven automations in natural language: "on file save, regenerate the relevant unit tests") and *Steering Files* (persistent project context, equivalent to AGENTS.md). Strength: hooks are a genuinely novel automation layer. Weakness: vendor lock-in to AWS/Bedrock.

**OpenSpec** (Fission AI, open-source, MIT) — the lightweight alternative. Deliberately minimal: no phase gates, no rigid artifact ordering. Separates `openspec/specs/` (current truth) from `openspec/changes/` (proposed updates). Works with 30+ AI assistants via slash commands. Strength: brownfield-first, low ceremony. Weakness: may be too thin for complex projects.

**BMAD Method** (~49K GitHub stars, MIT) — the heavyweight. 12+ specialized agent personas (Analyst, PM, Architect, Developer, UX, etc.) with full agile lifecycle: PRDs, architecture docs, epics, stories, sprints, QA gates. Code becomes a downstream derivative of specifications. Strength: complete lifecycle coverage. Weakness: steep learning curve, document-heavy, essentially rebuilds an entire agile team in prompts.

**TDAD** (Test-Driven AI Agent Definition, Fiverr Labs, 2026) — treats agent prompts as compiled artifacts. Engineers provide behavioral specifications, a coding agent converts them into executable tests, a second agent iteratively refines the prompt until tests pass. Notable for making the *prompt itself* the target of TDD.

**Academic research** (FSE 2026, Montreal) — a paper from McGill on "LLM-Assisted Repository-Level Generation with Structured Spec-Driven Engineering" confirms that natural language prompts are lossy due to inherent ambiguity, and that structured specifications (including Gherkin) significantly improve LLM code generation quality at repository scale.

## Specification Formats Compared

### 1. Gherkin (Given / When / Then)

The incumbent. Deterministic parser, strict grammar, broad tooling (Cucumber, Behave, pytest-bdd, SpecFlow/Reqnroll). Forces structured thinking.

```gherkin
Feature: Todo management
  Scenario: Add a todo item
    Given a user is logged in
    When they add "Buy milk"
    Then the todo list contains "Buy milk"
```

**Strengths for Craftsman Dev:**
- Executable specification — the spec *is* the test
- Deterministic parser means no ambiguity
- Mature cross-language tooling
- LLMs are well-trained on Gherkin (high-quality generation)
- Maps directly to ATDD workflow
- Human-readable even for non-developers

**Weaknesses for Craftsman Dev:**
- Verbose for complex scenarios — lots of boilerplate
- Step definitions are a maintenance burden — the translation layer between plain English and code
- Doesn't express non-functional requirements (performance, security)
- Doesn't express architectural constraints
- On Apple platforms specifically, UI behaviors map awkwardly ("Then the view displays a gradient background" — what does that mean mechanically?)
- The Given/When/Then constraint can feel forced for stateful or reactive systems

### 2. Gauge (Markdown-based BDD, ThoughtWorks)

Specifications written in markdown. Headings demarcate specs/scenarios. Bullet lists declare steps. Tables, code blocks, images — any markdown element is valid.

```markdown
# Customer can place an order

* Customer "Alice" is logged in
* The cart contains the following items:

  | Item     | Quantity | Price |
  |----------|----------|-------|
  | Widget A | 2        | 19.99 |

## Successful checkout
* When Alice completes checkout with card ending "1111"
* Then the order total should be "89.97"
* And an order confirmation should be sent
```

**Strengths for Craftsman Dev:**
- Markdown is the native format of AGENTS.md, PLAN.md, and ADRs — unified format across all artifacts
- More flexible than Gherkin's rigid grammar
- Specs double as documentation without conversion
- Plugin architecture supports Java, JS, Python, C#, Ruby, Go
- Built-in parallel execution
- Refactoring tooling

**Weaknesses for Craftsman Dev:**
- Community health warning — development activity has slowed since 2023
- Smaller ecosystem than Cucumber
- The flexibility that makes it attractive also makes it less strict — "some BDD purists argue Gauge's flexibility undermines the discipline of executable specifications"
- No Swift support
- No `craftsman verify` equivalent out of the box

### 3. Concordion (Living Documentation)

Specifications written as prose in HTML or Markdown, with instrumentation binding sentences to fixture code. Produces the most polished living documentation in the category.

```html
The full name <span concordion:set="#name">Jane Smith</span>
will be <span concordion:execute="#result = split(#name)">split</span>
into first name <span concordion:assert-equals="#result.firstName">Jane</span>
and last name <span concordion:assert-equals="#result.lastName">Smith</span>.
```

**Strengths for Craftsman Dev:**
- The specification document *is* the deliverable — beautiful living docs
- Prose reads naturally, not formulaically
- Hierarchical spec linking with automated breadcrumbs
- Results highlighted inline in the specification

**Weaknesses for Craftsman Dev:**
- Java-only (no Python, TS, Rust, Swift)
- HTML instrumentation is brittle and hard to maintain
- Niche adoption — "expect to train, not hire, for it"
- Requires embedding test commands in document markup
- Not a fit for cross-stack development

### 4. Property-Based Testing (Hypothesis, QuickCheck, fast-check)

Instead of specific examples, you describe the *space* of valid inputs and a *property* that must hold for all of them. The framework generates hundreds of random inputs to try to break the property.

```python
from hypothesis import given
from hypothesis.strategies import lists, integers

@given(lists(integers()))
def test_sort_preserves_length(xs):
    assert len(sorted(xs)) == len(xs)

@given(lists(integers()))
def test_sort_is_idempotent(xs):
    assert sorted(sorted(xs)) == sorted(xs)
```

Kiro's own blog explicitly states: "Property tests are a great match for specification-driven development because specification requirements are oftentimes directly expressing properties. In a sense, properties are another representation of (parts of) your specification."

**Strengths for Craftsman Dev:**
- Finds edge cases that example-based testing misses entirely
- Properties express *invariants* — stronger guarantees than examples
- Shrinking produces minimal failing cases for debugging
- Available in Python (Hypothesis), JS (fast-check), Rust (proptest), Swift (SwiftCheck)
- Complementary to BDD, not a replacement

**Weaknesses for Craftsman Dev:**
- Properties are harder to write than examples
- Not human-readable in the way Gherkin is — they're code, not specs
- Don't serve as living documentation
- Don't map to acceptance criteria that a human reviews
- Agent would need to write both properties *and* implementation — more room for the fox to guard the henhouse

### 5. Spec-Kit / Kiro Style (Structured Markdown)

Natural-language requirements in markdown, with optional user stories, acceptance criteria, and design constraints. No executable format — the spec guides implementation, and tests are written separately.

```markdown
## Requirements
### REQ-001: Add todo item
**User Story:** As a user, I want to add items to my todo list
**Acceptance Criteria:**
- User can enter a todo item with a title
- Item appears in the list after creation
- Empty titles are rejected with an error message
**Design Notes:**
- Use optimistic UI update
```

**Strengths for Craftsman Dev:**
- Maximum flexibility — any requirement shape fits
- No learning curve for the format
- Works with every AI agent without framework integration
- Natural language means the agent can reason about it directly

**Weaknesses for Craftsman Dev:**
- Not executable — you need a separate test suite
- Spec and tests can drift (the exact problem Gherkin solves)
- Natural language is ambiguous (the exact problem the FSE 2026 paper flags)
- No mechanical verification of spec-to-code alignment
- The human must verify that tests match the spec (or trust the agent to do it — which is the failure mode Craftsman Dev rejects)

### 6. Example Mapping (Discovery Technique)

A collaborative technique for discovering requirements before writing any specification. Uses colored cards: yellow (user stories), blue (rules), green (examples), red (questions). The output feeds into whatever specification format you use.

**Relevance for Craftsman Dev:** Not an alternative to Gherkin — it's a *complement* for the discovery phase. When the human and the agent-as-librarian are drafting SPEC.md, Example Mapping structures the conversation: "What are the rules? What are the examples? What questions remain?" This is the formalization of "talk through the spec before writing it."

## Synthesis: What Should Craftsman Dev Use?

### The Case for Staying with Gherkin

Gherkin's core value proposition — specification that is simultaneously human-readable and mechanically executable — is exactly what Craftsman Dev needs. The three-actor model requires that the machine can verify without opinion. Gherkin delivers this.

The LLM ecosystem is deeply trained on Gherkin. An expert agent writing scenarios from documentation will produce higher-quality Gherkin than any other format because there's more training data for it. This matters when the agent is the librarian.

The FSE 2026 paper confirms that structured specifications (including Gherkin) outperform natural language prompts at repository-scale code generation. Moving away from Gherkin toward less structured formats would be moving *against* the research.

### The Case for Augmenting Gherkin

Gherkin alone has gaps that the 2026 landscape has surfaced:

**Property-based testing catches what examples miss.** Gherkin scenarios are specific examples. They prove the code handles the cases you thought of. Property-based testing proves the code handles the cases you *didn't* think of. These are complementary, not competing. Adding Hypothesis/fast-check/proptest as a QA layer alongside Gherkin strengthens verification without abandoning the executable specification.

**Example Mapping structures the discovery phase.** Before the expert agent writes SPEC.md, an Example Mapping session between human and agent surfaces the rules, examples, and open questions. This is lightweight, costs almost nothing, and prevents the most common spec-writing failure: missing edge cases.

**Kiro's Hooks concept automates the boring work.** "On file save, re-run affected scenarios" or "on spec change, flag affected step definitions" — these event-driven automations could live in AGENTS.md or a `.hooks` file. Not essential, but worth noting as the kind of automation that reduces manual process.

**Gauge's markdown format is worth watching but not adopting.** The unified markdown ecosystem is appealing (AGENTS.md, SPEC.md, PLAN.md, ADRs all in markdown), but Gauge's slowing development and lack of Swift support make it a risk for a cross-stack methodology. If Gauge's community health stabilizes, it could replace Gherkin as the spec format with zero conceptual change to the workflow.

### The Case Against Switching

Every alternative to Gherkin either:

1. **Loses executability** (Spec-Kit/Kiro style markdown) — which breaks the three-actor model by requiring agent opinion for verification
2. **Loses readability** (property-based testing) — which breaks the librarian pattern by making specs code-only
3. **Loses maturity** (Gauge, Concordion) — which introduces tooling risk for cross-stack development
4. **Adds ceremony** (BMAD, full SDD frameworks) — which contradicts the lean philosophy

Gherkin with property-based augmentation gives you both: human-readable acceptance criteria that execute mechanically, plus invariant-checking that catches what examples can't.

## Recommended Architecture

```
Discovery (optional)
├── Example Mapping session with agent
├── Output: rules, examples, questions
└── Feeds into SPEC.md drafting

AGENTS.md (static, human)
├── Vision, design, architecture, tech stack
├── What good looks like
└── Loaded once per session

SPEC.md (static, Gherkin)
├── Drafted by expert agent from docs + Example Mapping output
├── Human-approved
├── Executable via craftsman verify
└── Augmented with property-based tests for invariants

PLAN.md (dynamic, agent)
├── Batched tasks targeting specific scenarios
├── Adapts after each batch
└── Success = craftsman verify --batch N exits 0
```

### What to Steal from the Competition

From **Kiro**: the Hooks concept. Event-driven automation ("on spec change, flag stale step definitions") is genuinely useful and could be a future craftsman CLI feature.

From **OpenSpec**: the separation of `specs/` (current truth) from `changes/` (proposed updates). For larger projects, this prevents spec drift during development.

From **BMAD**: nothing. Its 12-agent persona system is the antithesis of the craftsman approach. It rebuilds an entire agile team in prompts for people who don't have a team. You are the team.

From **Spec Kit**: the model-agnostic portability. Craftsman Dev should work with any agent, not just Claude Code.

From **Property-based testing**: the complementary QA layer. Not as a replacement for Gherkin, but as a mechanical amplifier that generates hundreds of test cases from invariant descriptions. The `craftsman verify` CLI could integrate both Gherkin scenario results and property test results in a single pass.

From **Example Mapping**: the structured discovery technique. When the expert agent drafts SPEC.md, the conversation should follow the Example Mapping structure: rules, examples, questions.

## Conclusion

Gherkin remains the right specification language for Craftsman Dev. It is the only format that simultaneously serves as human-readable requirements, machine-executable tests, and living documentation — the triple constraint that the three-actor model demands. The 2026 SDD wave confirms the value of structured specifications over natural language. Academic research confirms that Gherkin outperforms loose prompts for LLM-assisted development.

The enhancements worth adopting are complementary, not replacement: property-based testing for invariant coverage, Example Mapping for structured discovery, and eventually event-driven hooks for automation. The spec format stays Gherkin. The workflow stays lean. The verification stays mechanical.
