# Architecture Understanding & Agent Fan-Out/In: Research & Alternatives

> How agents understand and respect codebase architecture, and how parallel agent work is orchestrated — evaluated against the 2026 landscape of code intelligence, fitness functions, and multi-agent orchestration.

---

## Part 1: Architecture Understanding

### The Problem

When an agent modifies code without understanding the architecture, it produces changes that work locally but violate global structure. A 200K-line monolith study found that agents "confidently made changes across multiple layers, touched files they didn't need to touch, and broke parts of the system they never looked at." Every architectural violation the agent makes is a violation the human has to catch in review — or worse, catches in production.

The core insight from 2026: "Good architecture has always meant clear ownership, explicit contracts, obvious boundaries. We just used to let teams compensate for bad architecture through institutional knowledge. AI agents can't compensate. They expose."

### Three Mechanisms for Architecture Awareness

#### 1. Architecture Fitness Functions (Mechanical)

Architecture fitness functions are automated tests that verify the codebase adheres to architectural decisions. They are the architectural equivalent of Gherkin scenarios: mechanical, deterministic, pass/fail.

```java
// ArchUnit (Java): no circular dependencies
@Test
void no_circular_dependencies() {
    noClasses()
        .should().dependOnClassesThat()
        .resideInAnyPackage("..ui..")
        .from("..db..")
        .check(importedClasses);
}
```

Tools by ecosystem:
- **Java:** ArchUnit, jMolecules
- **TypeScript:** ArchUnitTS, dependency-cruiser
- **JavaScript:** dependency-cruiser, eslint-plugin-import
- **Python:** import-linter
- **.NET:** NetArchTest
- **Go:** depguard

Fitness functions enforce rules like:
- "No service class should depend on a controller class"
- "Domain modules must not import from infrastructure"
- "No circular dependencies between modules"
- "UI layer cannot import database layer directly"

These run as part of `craftsman verify` — they are tests, not guidelines. A fitness function violation is a build failure, just like a failing Gherkin scenario.

**For Craftsman Dev:** Fitness functions are the architectural equivalent of Gherkin scenarios. They belong in the verification stack alongside functional tests, lint, and health checks. They are defined during bootstrap (when writing AGENTS.md) and enforced mechanically at every batch boundary.

#### 2. Repository Maps (Agent-Side Context)

Aider pioneered the **Repository Map pattern**: tree-sitter parses code into an AST, extracts function signatures and class definitions, builds a dependency graph using PageRank to rank symbol importance, and dynamically fits optimal content within token budgets (~1,000 tokens for the map).

This gives the agent structural awareness of the entire repository without loading every file. The agent knows which modules exist, what their interfaces are, and how they relate — enough to respect boundaries without reading every implementation.

The 2026 evolution: tools like CodeGraph (47K stars), GitNexus (42K stars), and Serena build local-first knowledge graphs of codebases. The agent queries the graph for "what depends on this module?" or "what interface does this service expose?" before making changes.

Anthropic's guidance: "Just-in-time context, not pre-inference RAG — maintain lightweight identifiers, dynamically load data at runtime using tools." The agent doesn't need the whole codebase in context. It needs to know where to look and what constraints to respect.

**For Craftsman Dev:** The project's architecture section in AGENTS.md should include a module map with explicit boundaries and dependency rules. For larger projects, a code intelligence tool (dependency-cruiser, CodeGraph) provides the graph the agent queries before making cross-module changes.

#### 3. Architecture in AGENTS.md (Human-Defined Constraints)

AGENTS.md is where architectural invariants are encoded. The research converges on what belongs there:

```markdown
## Architecture

### Module Boundaries
- `src/domain/` — business logic, no external dependencies
- `src/infra/` — database, external APIs, framework integration
- `src/api/` — HTTP handlers, validation, serialization
- Domain must not import from infra or api
- Api imports domain but never infra directly

### Dependency Rules
- All cross-module communication through interfaces
- No circular dependencies (enforced by dependency-cruiser)
- New external dependencies require ADR

### Patterns
- Repository pattern for data access (see ADR-002)
- Event-driven for cross-module communication (see ADR-004)
```

The critical insight from the AGENTS.md research: "every improvement you make for your AI agents makes your codebase better for your human engineers too. Better boundaries. Clearer ownership. Explicit contracts."

**For Craftsman Dev:** Architecture constraints in AGENTS.md are hard constraints (HC tier from the implementation quality research). The agent must not violate them even if asked. Fitness functions verify mechanically. The ADR system records why boundaries were drawn where they are.

### Architecture-Aware Verification Stack

```bash
craftsman verify       # Gherkin scenarios (functional)
craftsman lint         # code style + token compliance
craftsman arch         # fitness functions (architectural)
craftsman health       # CodeScene structural quality
craftsman a11y         # accessibility (front-end)
craftsman visual       # screenshot regression (front-end)
```

`craftsman arch` runs architecture fitness functions — dependency rules, module boundary violations, circular dependency detection. A violation fails the build the same way a failing scenario does.

---

## Part 2: Agent Fan-Out/In

### The 2026 Landscape

Multi-agent orchestration has matured rapidly. Five distinct patterns dominate:

| Pattern | Topology | Use Case | Cost |
|---|---|---|---|
| **Supervisor** | Hierarchical delegation | Cross-domain tasks (code + research + review) | ~1.5× single |
| **Fan-out** | Parallel scatter-gather | Same task, multiple attempts or parallel subtasks | ~N× single |
| **Pipeline** | Sequential chain | Processing stages (analyze → plan → implement) | ~1× per stage |
| **Debate** | Multi-perspective critique | High-stakes decisions, design review | ~2.5× single |
| **Swarm** | Dynamic peer agents | Large-scale, open-ended exploration | ~N× single |

**Supervisor is the 2026 production default.** Claude Code subagents (one level deep), LangGraph Supervisor, and OpenAI Agents SDK handoffs all converge on this topology.

### How Fan-Out Works

The mechanical pattern:

1. **Dispatch:** the orchestrator creates N isolated contexts (git worktrees or subprocess contexts)
2. **Execute:** each agent works independently on its assigned task, with no shared state during execution
3. **Collect:** results return as summaries — the full execution context stays in the subagent's window, only the final output crosses the boundary
4. **Verify:** the orchestrator (or the human) evaluates results against acceptance criteria
5. **Merge:** approved results are merged into the main branch

The isolation is critical. Each subagent gets its own git worktree — a real, isolated working directory backed by one shared history. No file collisions. No race conditions. Each worktree is cleaned up automatically if the subagent makes no useful changes.

**The delegation prompt is everything.** The only channel from parent to subagent is the prompt string. If the subagent needs a file path, an error message, a branch name, or a decision already made in the main conversation, that information must be in the delegation prompt. Nothing crosses the boundary automatically.

### Cost Reality

A 3-agent team uses roughly **7× the tokens** of a single-agent session because each agent maintains its own context window. Reported large runs on Claude Code have reached $8,000 to $47,000. This is not a pattern to apply casually.

### What Craftsman Dev Should Adopt

#### Selective Fan-Out, Not Default Parallelism

Craftsman Dev's philosophy is "compress, don't spawn." Fan-out should be used only when the benefit clearly outweighs the cost:

**Use fan-out for:**
- **Research tasks** — spawning an isolated research agent to investigate an unfamiliar API or library while the implementation context stays clean. This is already in the methodology.
- **Best-of-N generation** — when a task has multiple plausible approaches, spawn N agents on the same task in parallel, test the results, and keep the best one. The orchestrate.sh pattern (fan out → test → rank → merge the winner) is a 50-line bash script.
- **Independent batch tasks** — when two batches have no dependencies, they can execute in parallel in separate worktrees. The batch boundary protocol runs independently for each.

**Don't use fan-out for:**
- **Sequential tasks within a batch** — tasks within a batch often have dependencies. Parallelizing them creates merge conflicts.
- **Code review** — a review agent needs the implementation context. Fan-out loses that context.
- **Bug diagnosis** — diagnosis requires tracing data flow through the actual codebase, not an isolated copy.

#### The Fan-Out Protocol for Craftsman Dev

```
Human identifies a fan-out opportunity
    │
    ├── 1. Define the scope for each agent
    │   └── Each agent gets: task description, relevant SPEC.md scenarios,
    │       AGENTS.md, and any docs needed
    │
    ├── 2. Create isolated worktrees
    │   └── git worktree add ../task-N feature/task-N
    │
    ├── 3. Dispatch agents in parallel
    │   └── Each works independently, no shared state
    │
    ├── 4. Verify results independently
    │   └── craftsman verify in each worktree
    │       └── Failed → discard worktree
    │
    ├── 5. Human reviews surviving results
    │   └── Chooses the best (for best-of-N)
    │       or merges all (for independent tasks)
    │
    └── 6. Cleanup
        └── git worktree remove ../task-N
```

#### Permanent Agents (Not Fan-Out)

Craftsman Dev already distinguishes permanent agents from disposable subagents. The fan-out pattern is for *disposable* parallel work. Permanent agents — code reviewer, research specialist — are long-running, named, and purposeful. They're not spawned per task; they're invoked on demand.

| Agent Type | Lifecycle | Context | Example |
|---|---|---|---|
| Main (librarian) | Per-batch, continuous | Full project context | Implementation |
| Research | Per-query, isolated | Question + docs only | API investigation |
| Code reviewer | On-demand, dedicated | AGENTS.md + diff | Architecture review |
| Fan-out worker | Per-task, disposable | Task prompt + worktree | Best-of-N attempts |

### What NOT to Adopt

**Default parallelism** — running every task in parallel because the tooling supports it. The token cost is 7× per additional agent. Most tasks don't benefit from parallelism; they benefit from focus and continuity. "Agentmaxxing" is a practice for large teams with high throughput needs, not for a craftsman workflow.

**Swarm patterns** — Kimi K2.6 scales to 300 agents. This is for massive exploration tasks. A craftsman working on a focused project has no use for swarm-scale orchestration.

**Cross-agent shared state during execution** — the research is clear that isolation during execution is what makes fan-out safe. Agents sharing files during parallel work produces race conditions and merge conflicts. Isolation first, merge after.

---

## Combined Verification Stack (Complete)

With architecture understanding and fan-out added, the full Craftsman Dev verification stack:

```
craftsman verify       # Gherkin scenarios (functional correctness)
craftsman lint         # code style + design token compliance
craftsman arch         # fitness functions (architectural boundaries)
craftsman health       # CodeScene structural quality
craftsman a11y         # axe-core WCAG scan (front-end)
craftsman visual       # Playwright screenshot regression (front-end)
```

All six gates are mechanical. All return exit codes. None involve LLM opinion. They compose into a single `craftsman check-all` that runs at batch boundaries and at the finish step.

## Conclusion

Architecture understanding and agent orchestration are two sides of the same coin: the agent must respect the project's structure, and when multiple agents work in parallel, each must respect that structure independently.

**Architecture awareness** is enforced through three mechanisms: fitness functions (mechanical tests for dependency rules and module boundaries), repository maps (agent-side structural context), and AGENTS.md (human-defined invariants). The agent doesn't need to understand the entire codebase — it needs to know the boundaries and respect them, verified mechanically.

**Agent fan-out** is a selective tool, not a default mode. The craftsman uses it for research isolation, best-of-N generation, and independent parallel batches — never for sequential work or review. Each fan-out agent works in an isolated git worktree, verified independently, and merged only after the human approves.

Both map to the three-actor model: the human defines architecture and fan-out scope, the agent implements within boundaries, and the machine verifies compliance mechanically.
