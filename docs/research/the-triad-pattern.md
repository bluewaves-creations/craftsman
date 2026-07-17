# The Triad Pattern: Instructions, Skills, CLI

> A complete architectural reference for designing agent systems around three primitives — instructions for strategy, skills for knowledge, and CLI for execution — and the design principles that make them work together.

---

## 1. Overview

The Triad Pattern is an architecture for AI agent systems built on three primitives, each with a single, non-overlapping responsibility:

- **Instructions** (directives, typically AGENTS.md) define *strategy* — how the agent should think, decide, and behave.
- **Skills** define *knowledge* — what the agent needs to know to perform a specific capability well.
- **CLI** defines *action* — deterministic operations the agent invokes through the shell.

The pattern replaces tool-protocol-centric architectures (such as MCP) with a design that is faster, more token-efficient, and more coherent — at the cost of requiring deliberate design judgment from the system builder.

The three primitives are not a stack; they are a triangle. Each one references and depends on the other two, but none subsumes another. Instructions orchestrate skills and CLI commands. Skills teach the agent when and how to use CLI commands. CLI commands execute what instructions and skills describe. The system's effectiveness comes from the separation, not from any single layer.


## 2. Instructions: The Strategy Layer

### 2.1 What Instructions Are

Instructions are the agent's standing orders. They define the operational strategy for a workspace, project, or domain: what the agent should do, in what order, under what conditions, with what priorities, and how it should handle ambiguity.

Instructions are typically encoded in an AGENTS.md file at the root of a workspace, though the name and location are convention-dependent. What matters is the role: instructions are the document the agent reads first to understand how to approach its work.

### 2.2 What Instructions Contain

Instructions encode several categories of strategic knowledge.

**Workflow logic.** The sequence of operations for common tasks. "When a new document arrives in the inbox, run intake, then classify, then file to the library." This is the procedural backbone that keeps the agent from improvising a plan on every turn.

**Decision criteria.** The rules for judgment calls. "If a source document has no date, use the file modification date. If ambiguous, ask." These reduce the reasoning load on the model by pre-resolving common decisions.

**Behavioral norms.** The tone, style, and posture the agent should adopt. "Sign every library action in the ledger. Never delete without confirmation. Prefer filing over deferring." These shape the agent's character within the workspace.

**Escalation rules.** The boundaries of autonomous action. "Proceed without confirmation for routine filing. Ask before merging documents. Never modify source files." These define the trust perimeter — what the agent can do alone and what requires human judgment.

**Skill references.** Pointers to which skills are available and when to load them. "For document conversion, load the doc-converter skill. For library operations, load the librarian skill." These give the agent a map of its capabilities without loading all capabilities at once.

**CLI references.** Documentation of available commands and when to use them. "Use `fusion register` to log actions. Use `fusion checks` to validate workspace integrity." These connect strategic intent to concrete execution.

### 2.3 What Instructions Do Not Contain

The boundary is as important as the content.

Instructions do not contain domain expertise. How to structure a well-formed library document, how to parse a PDF, how to handle edge cases in spreadsheet formatting — that knowledge lives in skills. Instructions say *when* to do something; skills say *how*.

Instructions do not contain implementation logic. The mechanics of file manipulation, data transformation, or system interaction live in CLI commands. Instructions say *what* to accomplish; the CLI says *what to run*.

Instructions do not duplicate CLI help text. If a CLI command has a `--help` flag that documents its parameters and options, instructions reference the command by name and purpose, not by repeating its documentation. Duplication creates drift; references stay current.

### 2.4 Design Principles for Instructions

**Be prescriptive, not descriptive.** Instructions tell the agent what to do, not what is possible. "Run intake on every new file" is an instruction. "The intake skill can process new files" is a skill description. The difference matters because descriptive text requires the agent to reason about whether and when to act; prescriptive text removes that reasoning step.

**Front-load the common path.** The most frequent operations should appear first. The agent reads instructions sequentially, and the most important guidance should occupy the most prominent position. Edge cases, fallbacks, and rare operations belong at the end.

**Encode the workflow as a narrative, not a flowchart.** Agents reason over prose more reliably than over structured diagrams or decision trees. "When a document arrives, first classify it by aurora. If it belongs in the library, file it and log the action. If it's ambiguous, place it in the workbench for review." This narrative form matches how language models process sequential logic.

**Keep instructions workspace-scoped.** Instructions describe how to work within a specific workspace, not general behavioral guidelines. An AGENTS.md for a research workspace will differ from one for a studio workspace. The specificity is the value — generic instructions produce generic behavior.

**Version alongside the workspace.** Instructions should live in the same repository or directory as the workspace they govern. When the workspace evolves, the instructions evolve with it. Storing instructions separately from the workspace they describe creates synchronization problems.


## 3. Skills: The Knowledge Layer

### 3.1 What Skills Are

A skill is a self-contained unit of domain knowledge that teaches an agent how to perform a specific capability well. Skills encode expertise: best practices, constraints, edge cases, vocabulary, patterns, and anti-patterns specific to a domain.

A skill is typically a SKILL.md file, optionally accompanied by templates, examples, or reference materials. The SKILL.md is the entry point — the document the agent reads to acquire the domain knowledge it needs for a task.

### 3.2 The Just-in-Time Property

Skills are loaded on demand, not permanently resident in the agent's context. This is the fundamental distinction from MCP tool schemas, which occupy prompt space on every turn regardless of relevance.

An agent working on a presentation loads the presentation skill. An agent converting a document loads the document conversion skill. When the task is complete, the skill's context cost is released. The prompt carries only the knowledge relevant to the current task.

This just-in-time injection means the system's total capability can grow without bound while the per-turn context cost remains proportional to the active task. Adding a new skill to the system adds zero tokens to prompts that don't use it.

### 3.3 What Skills Contain

**Domain expertise.** The accumulated knowledge of how to do something well. For a spreadsheet skill, this includes formatting conventions, formula patterns, chart types, and data validation approaches. For a PDF skill, this includes text extraction methods, form-filling techniques, and merge/split operations. The expertise should reflect production-grade knowledge, not introductory tutorials.

**Constraints and guardrails.** What the agent should avoid or handle carefully. File size limits, format restrictions, platform-specific behaviors, known failure modes. These prevent the agent from attempting operations that will fail or produce poor results.

**Tool and library guidance.** Which libraries to use, which to avoid, and how to configure them. A skill might specify "use python-docx for Word documents, not pandoc" or "always install with --break-system-packages." This is operational knowledge that prevents the agent from making suboptimal tooling choices.

**Output quality criteria.** What a good result looks like. A document creation skill might specify formatting standards, structural requirements, or validation checks. These give the agent a target to aim for and a basis for self-evaluation.

**Examples and templates.** Concrete instances of correct output. A template for a library document's frontmatter, an example of a well-structured report, a sample CLI invocation with expected output. Examples reduce ambiguity more effectively than prose descriptions.

### 3.4 What Skills Do Not Contain

Skills do not contain workflow logic. *When* to create a spreadsheet versus a Word document is an instruction concern. The skill assumes the decision has already been made and teaches the agent how to execute it well.

Skills do not contain behavioral norms. Whether the agent should ask for confirmation, how it should handle errors, what tone it should use — these are instruction concerns. A skill is domain knowledge, not personality.

Skills do not contain CLI command implementations. A skill might reference CLI commands ("use `fusion register` to log this action"), but the command itself is a separate artifact. Skills teach; CLI commands do.

### 3.5 Design Principles for Skills

**One skill per capability.** A skill should cover one coherent domain. A spreadsheet skill and a presentation skill are separate skills, even if they share some underlying technology. Combining them creates a large context payload for tasks that need only half of it.

**Teach the expert path.** Skills should encode how an expert performs the task, not how a beginner learns about it. The agent is operating, not studying. Explanatory context should be minimal; operational guidance should be comprehensive.

**Include the failure modes.** The most valuable knowledge in a skill is often what goes wrong and how to handle it. Known bugs, platform quirks, format limitations, edge cases — these save more tokens (in wasted attempts and retries) than they cost (in skill document length).

**Keep skills stable.** A skill should change rarely. If it changes frequently, the volatile parts likely belong in instructions (which are workspace-specific and expected to evolve) rather than in the skill (which is shared across workspaces and expected to be stable).

**Make skills portable.** A well-designed skill can be used by any agent that reads the Agent Skills format — not just one specific model or platform. Avoid model-specific prompt engineering within skills. The knowledge should be expressed in clear, unambiguous prose that any capable model can follow.


## 4. CLI: The Execution Layer

### 4.1 What the CLI Is

The CLI is a set of command-line tools that the agent invokes through bash to perform deterministic operations. It is the execution surface of the triad — the layer where intent becomes action.

The CLI replaces structured tool protocols (MCP, function calling) with a simpler mechanism: the agent runs a shell command and reads the output. The interface is text in, text out, mediated by the most universal tool interface in computing.

### 4.2 Why CLI Over Tool Protocols

The substitution of CLI for MCP is not merely a preference; it has structural consequences.

**Zero prompt overhead.** CLI commands do not require schema definitions in the agent's prompt. The entire command surface exists outside the context window until the moment a command is invoked. Adding ten CLI commands adds zero tokens to any prompt. Adding ten MCP tools adds ten schema blocks to every prompt.

**Lazy-loaded documentation.** When the agent needs to discover available commands or understand a command's parameters, it runs `--help`. This costs output tokens only when needed, compared to MCP schemas which cost input tokens on every turn.

**Minimal invocation overhead.** A CLI call is `bash` → process → stdout. An MCP call is schema interpretation → JSON-RPC construction → serialization → transport → deserialization → result parsing. The difference in latency is significant for high-frequency tool use.

**Shared human-agent interface.** The developer can run the same commands the agent runs, inspect the same outputs, and debug the same failures. There is no protocol layer that the human must simulate or translate through.

**Trivial implementation.** A new CLI command is a script in a bin directory. A new MCP server is a running process with JSON-RPC transport, schema definitions, and often authentication infrastructure. The distance from identifying a need to deploying a capability is radically shorter with CLI.

**Scalability without cost.** The system's capability surface can grow without bound. Every new command is free in context terms until the moment it is used.

### 4.3 What the CLI Handles

The CLI handles operations that are deterministic, repeatable, and benefit from speed. These are tasks where the agent's judgment is not the bottleneck — the mechanics of execution are.

**File operations.** Creating, moving, renaming, validating files. These are mechanical operations that should execute in milliseconds, not consume reasoning tokens.

**Registration and logging.** Appending to ledgers, writing audit trails, recording metadata. These are bookkeeping operations that must be reliable and fast.

**Validation and checks.** Verifying workspace integrity, checking frontmatter compliance, detecting drift. These are rule-based operations that should execute deterministically, not probabilistically.

**Scaffolding.** Creating directory structures, writing initial file templates, setting up new workspaces. These are generative operations with known outputs that should not require model inference.

**Composition and aggregation.** Combining files, generating indexes, computing cross-references. These are data operations that are faster and more reliable as code than as model reasoning.

### 4.4 What the CLI Does Not Handle

The CLI does not handle tasks that require judgment, interpretation, or language generation. These belong to the agent, guided by skills and instructions.

**Classification.** Deciding what aurora label a document should carry requires understanding the document's content and the user's attention priorities. This is agent work.

**Summarization.** Producing a meaningful summary of a source document requires comprehension and editorial judgment. This is agent work.

**Curation.** Deciding whether two documents should be merged, whether a document is still relevant, whether a library needs reorganization — these are judgment calls that require contextual understanding.

**Communication.** Composing messages, explaining decisions, asking clarifying questions — anything involving natural language generation is agent work.

The principle is clear: if the operation has one correct output for a given input, it belongs in the CLI. If the operation requires judgment about what the output should be, it belongs with the agent.

### 4.5 Design Principles for CLI Commands

**One command, one job.** Each command should do exactly one thing. `fusion register` logs an action. `fusion checks` validates integrity. Commands that do multiple things create ambiguity about what the agent is invoking and make error handling harder.

**Explicit over implicit.** Commands should require explicit parameters rather than inferring context. `fusion register --action filed --target doc.md` is better than `fusion register` with implicit state. Explicit invocation is debuggable, auditable, and reproducible.

**Structured output for machine consumption.** When a command produces output that the agent will parse, use a consistent structured format — JSON, YAML, or well-defined plain text. The agent should not need to interpret prose from a CLI command; prose interpretation is expensive and error-prone.

**Human-readable output for human consumption.** When a command produces output that a human will read (help text, status messages, error reports), use clear, readable prose. The notary voice — present, accountable, never pretentious — is a good model.

**Discoverable help.** Every command should support `--help` with clear documentation of purpose, parameters, and examples. This is the agent's documentation surface — the lazy-loaded alternative to MCP schemas. The quality of help text directly affects the agent's ability to use the command correctly.

**Fast execution.** CLI commands should complete in milliseconds to low seconds. If a command takes longer, it should provide progress indication. Agent workflows often chain multiple CLI calls in a single reasoning step; slow commands break the flow.

**Idempotent where possible.** Commands that can be safely re-run produce more robust agent workflows. If the agent is uncertain whether a command succeeded, it should be able to retry without side effects.


## 5. The Interplay: How the Three Layers Work Together

### 5.1 The Information Flow

The three layers form a cycle, not a hierarchy.

Instructions tell the agent *what to do*: "When a new file arrives in the inbox, process it through intake." The agent reads this and understands the workflow.

Skills tell the agent *how to do it*: "To process a document through intake, extract the metadata, classify by type, generate a summary, and structure the output with these frontmatter fields." The agent reads this and understands the domain.

The CLI *does it*: `fusion register --action intake --source inbox/doc.pdf --target library/doc.md` logs the action in the ledger. The agent runs this and the operation is recorded.

The agent is the intelligence that connects the three layers. It reads instructions to understand strategy, loads skills to acquire expertise, and invokes CLI commands to execute operations. Its reasoning bridges the gaps between what to do, how to do it, and the mechanical act of doing it.

### 5.2 The Separation of Concerns

Each layer's boundary is defined by a simple test.

**Does it change when the workspace changes?** → It belongs in instructions. A research workspace has different workflows than a studio workspace. Instructions are scoped to context.

**Does it change when the domain changes?** → It belongs in a skill. Processing PDFs requires different knowledge than processing spreadsheets. Skills are scoped to capability.

**Does it change when neither changes?** → It probably doesn't change at all, and belongs in the CLI. Registering an action in a ledger is the same operation regardless of workspace or domain. CLI commands are scoped to mechanics.

When something seems to belong in two layers, the test usually resolves the ambiguity. "Use the librarian skill for library operations" is an instruction (workspace-specific strategy). "To add a document to the library, validate the frontmatter, check for duplicates, and place it in the correct subdirectory" is a skill (domain-specific expertise). "fusion checks --zone library" is a CLI command (mechanical validation).

### 5.3 The Anti-Patterns

**Instructions that duplicate skill content.** If instructions contain detailed how-to guidance, they are doing the skill's job. Instructions should reference skills, not replicate them. Duplication creates maintenance burden and risks drift between the two copies.

**Skills that contain workflow logic.** If a skill says "first do A, then do B, then do C based on the workspace state," it is doing the instructions' job. Skills should teach capabilities independent of workflow context. The same skill should be usable across different workspaces with different workflows.

**CLI commands that require judgment.** If a CLI command needs to interpret ambiguous input, classify content, or make editorial decisions, it is doing the agent's job. CLI commands should be deterministic. If the operation requires judgment, the agent should make the judgment and pass the result to the CLI as an explicit parameter.

**Instructions that embed CLI invocations as rigid scripts.** Instructions should describe intent and workflow, not prescribe exact command lines. "Register every library action in the ledger" is good. "Run `fusion register --action filed --target $FILE --timestamp $(date -u +%Y-%m-%dT%H:%M:%SZ)`" is too rigid — it breaks when the CLI evolves and prevents the agent from adapting the invocation to context.

**Skills that replicate CLI help text.** If a skill documents a CLI command's parameters and options, it is duplicating information that `--help` already provides. Skills should describe when and why to use a command, not how its flags work.


## 6. The Scaffold Pattern

### 6.1 Self-Bootstrapping

The triad's most powerful property is its ability to bootstrap itself. A skill can create an entire working environment in one operation:

1. **Create the directory structure.** The workspace layout — zones, subdirectories, initial files — is generated according to the convention.
2. **Write the instructions.** The AGENTS.md file is generated with references to the relevant skills and CLI commands, pre-configured for the workspace's purpose.
3. **Expose the CLI.** The workspace is immediately operable because the CLI is already installed and the instructions already reference it.

The result is a workspace that is ready to use from the first moment. The agent can read the instructions, load the referenced skills, and begin operating. There is no setup phase, no configuration step, no "connect your tools" workflow.

### 6.2 Why Scaffolding Matters

Scaffolding eliminates the gap between "I need a new workspace" and "the workspace is operational." In MCP-based architectures, this gap is filled with manual tool connection, configuration, and prompt engineering. Each new workspace requires bespoke setup.

With the triad, new workspace creation is a single skill invocation that produces a complete, functional system. The skill encodes the design judgment that would otherwise be required from the user — what structure to create, what instructions to write, what conventions to follow.

This also means the design quality is consistent. Every workspace created by the same scaffold skill follows the same conventions, uses the same instruction patterns, and references the same CLI commands. The system's coherence is a property of the scaffold, not a property of the user's discipline.

### 6.3 The Implication

The scaffold pattern means the triad is not just an architecture — it is a generative system. It can produce instances of itself. A skill that creates workspaces is a skill that creates environments where skills, instructions, and CLI operate together. The architecture reproduces.

This is a property that component-based architectures (connect these MCP servers, write this system prompt, configure these tools) fundamentally lack. They assemble; the triad generates.


## 7. Context Economics

### 7.1 The Token Budget

Every agent operates within a fixed context window. The token budget for any given turn must cover: the system prompt, instructions, loaded skills, conversation history, retrieved context, and the model's reasoning. Everything competes for the same space.

The triad minimizes the fixed costs. Instructions are concise by nature — they describe strategy, not encyclopedia entries. Skills load on demand and release when done. CLI commands consume zero prompt tokens until invoked, and then only the output tokens matter.

### 7.2 Comparison with MCP

In an MCP-based architecture, every connected tool's schema is present in every prompt. A moderately equipped agent with twenty tool connections might dedicate 3,000–5,000 tokens to tool schemas alone — tokens that are paid on every turn regardless of tool use.

In the triad architecture, the same capability surface costs zero tokens when no tools are in use, and costs only the tokens of the specific `--help` output when discovery is needed. For a typical agent session where tools are used on a minority of turns, the savings compound across the entire conversation.

### 7.3 The Scaling Property

As the system's capabilities grow, the difference widens.

With MCP, adding capabilities increases the per-turn context cost linearly. Fifty tools means fifty schemas in every prompt. The system becomes less effective as it becomes more capable.

With the triad, adding capabilities has zero impact on per-turn context cost. Fifty CLI commands is the same prompt cost as five. The system scales without degradation.

This scaling property is not a marginal efficiency gain. Over long conversations with many turns, the cumulative token savings translate directly into faster inference, lower cost, and more space for the content that actually matters — the user's context and the agent's reasoning.


## 8. Implementation Considerations

### 8.1 Language Choice

CLI commands can be implemented in any language. The choice should be driven by the complexity of the operation.

Shell scripts (bash, zsh) are appropriate for simple orchestration: file manipulation, command chaining, environment setup. They are fast to write, require no compilation, and are universally available.

Compiled languages (Swift, Go, Rust) are appropriate for complex operations that benefit from type safety, structured data handling, or performance requirements. They produce static binaries that launch in milliseconds and have no runtime dependencies.

The gradient from shell to compiled language should be driven by complexity, not premature optimization. Most CLI commands start as shell scripts. Some graduate to compiled binaries as they accumulate logic. The interface (command name, flags, output format) remains the same regardless of implementation language.

### 8.2 Help Text as Documentation

The `--help` output of each CLI command is the agent's primary documentation surface. Its quality directly determines how effectively the agent can use the command.

Good help text includes: a one-line description of the command's purpose; a usage synopsis with parameter names; a brief description of each parameter; at least one example invocation; and the expected output format.

Help text should be written for an audience that includes both humans and language models. This means: no abbreviations without expansion, no jargon without definition, and no implicit assumptions about context.

### 8.3 Error Handling

CLI commands should produce clear, structured error output that the agent can parse and act on. An error message like "Error: frontmatter field 'aurora' missing in document.md" is actionable. An error message like "Validation failed" is not.

Error output should include: what went wrong, which input caused it, and (where possible) what to do about it. The agent should be able to read an error message and either fix the problem or report it to the user with sufficient context.

### 8.4 Testing the Boundaries

The best test for whether the triad's boundaries are correct is to attempt a violation and see what breaks.

Put workflow logic in a skill → it becomes workspace-specific and loses portability.

Put domain expertise in instructions → they become bloated and slow to parse.

Put judgment in a CLI command → it becomes unpredictable and hard to debug.

Put CLI mechanics in instructions → they drift from the actual implementation.

Each violation produces a specific, observable failure. The architecture is self-correcting in the sense that misplacement creates friction that signals the error.


## 9. Summary

The Triad Pattern separates agent system design into three primitives with non-overlapping responsibilities:

| Layer | Responsibility | Changes when | Loaded |
|---|---|---|---|
| Instructions | Strategy, workflow, orchestration | Workspace changes | Always (small) |
| Skills | Domain knowledge, expertise | Capability changes | On demand |
| CLI | Deterministic execution | Rarely | Never (in prompt) |

The pattern produces agent systems that are faster (no protocol overhead), cheaper (minimal context cost), more coherent (explicit strategy layer), and more scalable (capability grows without context cost) than tool-protocol-centric architectures.

The tradeoff is design judgment. The pattern requires the system builder to make deliberate decisions about what belongs in each layer, to write clear instructions, to encode genuine expertise in skills, and to build reliable CLI tools. It rewards craftsmanship and punishes thoughtlessness.

For practitioners willing to invest in design, the returns are substantial. The system works. And it scales.

---

*The Triad Pattern is implemented in [Fusion](https://github.com/bluewaves-creations/fusion), an open-source working environment built on these principles. The [Agent Skills](https://agentskills.io) open standard defines the portable format for skills.*
