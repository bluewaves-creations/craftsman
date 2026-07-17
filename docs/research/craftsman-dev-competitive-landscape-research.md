# The Competitive Landscape: An Honest Audit

> Are we reinventing the wheel, and where would Craftsman Dev NOT win? A survey of the July 2026 landscape — methodology frameworks, harness built-ins, verification orchestrators, skill families, and agentic platforms — with a reuse-vs-build matrix, a stress-tested differentiation thesis, and the bear case. All claims verified against live sources on 2026-07-17 unless marked UNVERIFIED.

---

## The Question

The team's bar: best-in-class performance, efficiency, and output quality, outperforming any competing approach. That bar cannot be cleared by assertion. This document audits every neighboring project — what it does better than us today, what we should reuse instead of rebuild, and where Craftsman Dev would honestly lose. The short answer: the *combination* is unoccupied, several *components* are crowded, and two incumbents are further along than we'd like to admit.

## Survey 1: Methodology Frameworks

| Framework | State (2026-07) | Core idea | Verdict |
|---|---|---|---|
| Superpowers (obra) | v6.1.1 Jul 2026, ~257k stars, ~10 harnesses | Full SDLC as skills: brainstorm → plan → subagent TDD → two-stage review | **Compete + steal** |
| BMAD Method | V6.10.0 Jul 2026, 50.7k stars | 12+ agile personas, full lifecycle, now scale-adaptive with skills architecture | **Ignore** |
| GitHub Spec Kit | v0.13.0 Jul 2026, 122k stars, active | Constitution → specify → plan → tasks → implement; 30+ agents | **Steal design** |
| OpenSpec | v1.6.0 Jul 2026, 61.4k stars | Change-delta specs, `/opsx:` workflow, lightweight | **Steal design** (already adopted in brownfield doc) |
| AWS Kiro | GA May 2026; replaced Amazon Q Dev | requirements.md (EARS) → design.md → tasks.md; Hooks; spec-mode credits cost 5x vibe-mode (UNVERIFIED pricing, third-party) | **Steal hooks concept; compete on spec** |
| Tessl | Framework still closed beta; Registry pivoted to "Skills Registry" (May 2026) | Spec-as-source retreating; now an agent-skills package manager | **Ignore; watch the registry** |
| Amp (Sourcegraph) | Very active; Dial modes, orbs, cross-thread agent comms (Jul 2026) | Threads as artifacts, hierarchical AGENTS.md, Oracle reviewer, isolated subagents | **Steal thread-hygiene ideas** |
| aider | v0.86.0 Aug 2025 — maintenance mode; "Where is Paul?" issues open | Pioneered CONVENTIONS.md, repo maps, architect/editor split | **Ignore (honor the ancestors)** |
| Vibe/agentic engineering (Willison, Hashimoto, Ronacher) | Active essays + "Agentic Engineering Patterns" guide (2026) | AI multiplies classic practices; outer harness loop; always-an-agent-running | **Adopt vocabulary and patterns** |
| Gherkin/BDD agent frameworks | ~16 repos, 1–3 stars each; no funded player found | Andy Knight's gherkin-guidelines.md (Apr 2026) is the closest artifact | **The niche is empty — build** |

### The frameworks in detail

**Superpowers** (Jesse Vincent, now under Prime Radiant Inc., MIT). The methodology incumbent: brainstorm → written plan of 2-5-minute tasks → subagent-driven TDD (RED-GREEN-REFACTOR) → multi-stage review → verification-before-completion, all as composable skills across ~10 harnesses. The 4.x release (Dec 2025) is the one to study: it split review into a **spec-compliance agent** (does the implementation match the plan?) and a separate code-quality agent, both running formal fix-request loops; consolidated skills to fit harness character limits; rewrote every description to say *when* to trigger; and added e2e tests of the methodology itself. 5.x improved worktree handling; 6.x (Jun 2026) is explicitly a token-economy release. Weaknesses: heavyweight on small tasks (acknowledged, being addressed), and every quality verdict is an LLM opinion — including the spec-compliance check, which is precisely the step Craftsman makes mechanical.

**GitHub Spec Kit** — still shipping (v0.13.0 released the day of this audit), so "SDD is dead" takes are wrong. But the criticism record is unusually well documented: Böckeler (martinfowler.com) on verbose interrelated markdown and review fatigue; Scott Logic's "reinvented waterfall?"; an open repo discussion titled "SpecKit creates the illusion of work." Community consensus: it earns its cost on multi-PR, multi-engineer features and burns roughly 2x OpenSpec's tokens (UNVERIFIED third-party benchmark). Lesson for Craftsman: artifact count is the enemy — two files, not five.

**AWS Kiro** — went GA May 2026 and displaced Amazon Q Developer as AWS's lead dev tool (new Q signups closed May 15, 2026, per third-party reporting — UNVERIFIED against aws.amazon.com). Its EARS-notation requirements → design → tasks flow is the only harness-native spec lifecycle. Two details matter: Agent Hooks (event-driven automations, widely considered its best idea, free of quota) and the controversial credit pricing where spec-mode interactions cost 5x vibe-mode — AWS is literally charging a spec tax and justifying it with a claimed 23-37% downstream error reduction (UNVERIFIED, third-party). Reception is mixed: praised in regulated contexts, mocked for producing 4 user stories and 16 acceptance criteria for a small bugfix.

**Tessl** — the cautionary tale. $125M raised on spec-as-source; as of July 2026 the Framework has still not shipped GA (closed beta, JavaScript-only, per a third-party hands-on review — UNVERIFIED in detail), and the company pivoted its only production component into a "Skills Registry" / agent-enablement platform. The non-deterministic-compiler problem — same spec, different code every run — is unsolved, and Tessl's retreat is the market conceding it. Craftsman's stance (spec *verifies* code, never *generates* it) is the correct side of this divide.

**Amp** — less a competitor than a source of workflow doctrine: threads as shareable version-controlled artifacts, one-thread-per-task, hierarchical AGENTS.md, an Oracle (separate reasoning model invoked explicitly for hard review), subagents deliberately unable to talk to each other for context hygiene — though the Jul 17, 2026 release loosened that with cross-thread agent communication. Also shipped its own Skills system and remote headless "orbs." Proprietary and pay-per-use; its ideas travel, its platform doesn't.

**aider** — effectively unmaintained (last release Aug 2025; "Where is Paul?" and succession-plan issues open; leaderboard stale since Nov 2025). Historically important as the ancestor of half the field's conventions (CONVENTIONS.md → AGENTS.md, repo maps, architect/editor split). Its fate is a reminder that single-maintainer methodology tools die fast — relevant to a team-only project's bus factor.

**What the survey proves.** Spec-driven development is a crowded, criticized category — Böckeler's Thoughtworks analysis (verbose interlinked markdown, review fatigue, "aspires to spec-anchored but functions as spec-first"), Scott Logic's "reinvented waterfall?", and Kiro's 16-acceptance-criteria bugfix are the canonical failure modes. But **no framework makes Gherkin the executable spec that a machine verifies with exit codes**. Spec Kit, OpenSpec, Kiro, and BMAD all produce prose specs an LLM interprets; verification of "does the code match the spec" is either human review or (Superpowers 4.x) *another LLM* — a spec-compliance review agent. Craftsman's core bet — the spec compiles to tests, the machine says pass/fail — has no incumbent. That is simultaneously encouraging and a warning: either nobody thought of it, or the step-definition maintenance cost is why nobody shipped it. Ürgo Ringo's Specification-by-Example experiment (Nov 2025) found exactly that: good work-chunking, doubtful glue-code ROI.

**Superpowers deserves specific respect.** 4.x (Dec 2025) split review into spec-compliance + code-quality loops; 6.x (Jun 2026) is a token-economy release ("much faster, many fewer tokens, same quality"). Its trajectory — efficiency, adversarial review prompts, e2e tests *of the methodology itself* — is the discipline Craftsman must match. Its weakness is ours to exploit: every Superpowers verdict is still an LLM opinion.

## Survey 2: Harness Built-ins — What Mid-2026 Already Absorbed

Verified against official docs/changelogs on 2026-07-17.

| Capability | Claude Code v2.1.212 | Codex CLI v0.144.5 | Cursor 3.11 | Gemini CLI |
|---|---|---|---|---|
| Plan mode | Mature + ultraplan (cloud, commentable, saves to file) | `/plan`, `/goal` with verification criteria | Mature; plans persist as workspace files; parallel plans | Experimental |
| Subagents/orchestration | Nested, background-first, JS dynamic workflows (up to 1,000 agents) | Max/Ultra subagent modes | 8 parallel agents, worktrees | Experimental (+ remote A2A) |
| Hooks (hard gates) | Full lifecycle | **None documented** | Richest surface (~20 events, LLM-evaluated hooks) | Early |
| Skills | Native (originated standard) | Native, `.agents/skills` | Native + reads Claude/Codex dirs | Native, default-on |
| Memory | CLAUDE.md + rules/ + **auto memory on by default** | Memories + Chronicle | No documented auto-memory (UNVERIFIED absence) | Experimental |
| Checkpoints/rewind | Mature (`/rewind`, per-prompt) | Not documented | Not documented | Native |
| Code review | `/code-review` + managed service with mechanical verification pass, severity JSON for CI | `/review` + GitHub auto-review | BugBot lineage; iOS PR review | None |
| AGENTS.md | **Not native** (import/symlink) | Deep hierarchical support | Native | GEMINI.md |

### What the year absorbed

The pace matters more than any single feature. In roughly twelve months Claude Code went from a chat loop to: ultraplan (cloud planning sessions with inline comments on plan sections, plans persisted to disk on cancel), auto memory **on by default** (self-maintained MEMORY.md plus topic files, loaded every session), per-prompt checkpoints with `/rewind` across `/clear`, dynamic workflows (Claude writes a JavaScript orchestration script — `agent()`, `pipeline()`, up to 1,000 agents — saved as reusable commands), stacked skill invocation, `/doctor` proposing CLAUDE.md trims and flagging unused skills, and a managed Code Review service whose findings pass through **a mechanical verification step that checks candidates against actual code behavior**, emitting severity-tagged JSON a CI gate can parse. That last item is the single most important competitive fact in this document: Anthropic is already building mechanical verification *into review*. It validates the thesis and crowds one corner of it.

Codex counters differently: the deepest AGENTS.md implementation (hierarchical per-directory discovery, overrides, 32 KiB budget), native `/goal` objects carrying **verification criteria** with pause/resume — the closest any harness comes to a native definition-of-done — but no user-scriptable hook system at all, which means Craftsman's gates have no native enforcement point there and must run as plain CLI calls. Cursor 3.x has the richest hook surface (~20 events, including LLM-evaluated hooks and pre-read/pre-MCP gates), plans that persist as workspace files, and — via the Graphite acquisition — an in-house review product. Gemini CLI trails: skills are native and default-on, but plan mode, subagents, and auto memory are all still experimental, making it the harness where an external methodology adds the most.

**Redundant if we build it:** session memory, context compaction (Claude Code re-injects CLAUDE.md post-compact and offers targeted summarization), checkpointing, plan-generation UX, multi-agent orchestration, skills packaging. Craftsman's compaction-by-extraction design survives only as "extract durable learnings to the repo" — the session-state half is now worse than native.

**Genuinely open on every harness:** (1) durable spec files as versioned artifacts of record (only Kiro has a native spec lifecycle); (2) opinionated mechanical pass/fail gate *policies* — every harness ships enforcement points (hooks, `/goal` criteria), none ships the gates; (3) git-trailer provenance in repo history; (4) docs grounding/curation; (5) cross-harness consistency — the strongest remaining value proposition, and Agent Skills (~40-45 adopters incl. all four majors) makes it deliverable.

**Integration map — where each Craftsman component plugs into native features rather than competing:**

| Craftsman component | Claude Code integration | Codex | Cursor | Gemini CLI |
|---|---|---|---|---|
| Gate enforcement | PreToolUse/Stop hooks call `craftsman check` | None — plain CLI in AGENTS.md workflow | Hooks (richest surface) | Hooks (early) |
| PLAN.md batches | Produced via plan mode/ultraplan, persisted by us | `/goal` carries batch success criteria | Plan files saved to workspace | Manual |
| SPEC.md authoring | Skill (native) | Skill via `.agents/skills` | Skill | Skill |
| Verify-at-done | Stop hook + `/code-review` severity JSON | `/goal` verification criteria → `craftsman verify` | afterFileEdit/stop hooks | Manual invocation |
| Ledger trailers | Commit flow in skill + CLI writes trailers | Same | Same | Same |
| Docs grounding | `craftsman docs` cache + CLAUDE.md pointer | AGENTS.md pointer | rules/skill pointer | GEMINI.md pointer |

The Codex column is the sobering one: with no hook system, enforcement there is purely conventional — the agent must *choose* to run the gates. This is an argument for the ledger's `Verified-by:` trailer (post-hoc auditability) and for CI as the backstop enforcement point on every harness.

## Survey 3: Verification & Quality Orchestrators

| Tool | State (2026-07) | Unifies mechanical gates? | Verdict |
|---|---|---|---|
| trunk check | plugins v1.10.2 Jun 2026; company pivoted to flaky-tests/CI | Lint+security+format, hold-the-line; static only | **Steal architecture** (already the model for `craftsman check`) |
| qlty | v0.635.0 Jul 17 2026, Rust, 70+ plugins, free | Widest static coverage + maintainability + coverage; no runtime gates | **Wrap or steal** — closest static-side base |
| MegaLinter | v9.6.0 Jun 2026 | CI-heavyweight static aggregator | Ignore (Docker-first, slow inner loop) |
| pre-commit / lefthook | 4.6.0 / v2.1.10 Jul 2026 (lefthook added AI-agent integration) | Generic runners, no gate semantics | **Reuse as hook wiring**, not as brain |
| Danger / Reviewdog | Frozen-stable / slow (+2025 supply-chain incident) | PR reporters, not orchestrators | Ignore |
| CodeRabbit / Greptile / Graphite | $40M ARR, free CLI / semantic graph / **acquired by Cursor Dec 2025** | Probabilistic review, not mechanical gates | **Coexist** — AI review finds what gates can't; never a pass/fail authority |
| CodeScene | Code Health MCP server; "AI code fails 60%+ more in unhealthy code" research | Single-dimension health gate | Steal the trend-gate idea (fail on decline = our ratchet) |
| SonarQube 2026.2 | "Fight AI Slop"; AI Code Assurance, model-agnostic CodeFix | Server-side static quality gate | Ignore (server model, enterprise paywall) |
| Qodo 2.0 | $70M Series B Mar 2026 on "code verification" | Multi-agent review — probabilistic | Coexist |
| ArchUnit family / dependency-cruiser | ArchUnit 1.4.x, dependency-cruiser v18.1.0 very active | Per-language arch rules; **Rust/Swift are gaps** (UNVERIFIED absence) | **Wrap per-language; build Swift/Rust rules ourselves** |

### Positioning notes

**trunk check**, already the architectural model for `craftsman check` (per the verification-cli doc), needs a status update: the company's attention has visibly moved to flaky-test detection and CI analytics — the Code Quality product still ships (plugins v1.10.2, Jun 2026; ESLint's own repo uses it) but reads as maintenance-mode, and its launcher is closed source. No acquisition found (UNVERIFIED absence). That shifts the wrap-candidate calculus toward **qlty**: same meta-linter category, Rust, shipping same-day releases as of Jul 17, 2026, 70+ tools, adds maintainability metrics and coverage that trunk lacks, free under Fair Source/BUSL with delayed open-sourcing — but pre-1.0 and not OSI-open. Either way, Craftsman should not re-implement linter installation, version pinning, or hold-the-line for the lint/security gate; that wheel exists twice over.

**The AI-review market consolidated hard** and is worth watching precisely because it is converging on gates from the opposite direction: CodeRabbit ($40M ARR Apr 2026 per Sacra estimate — UNVERIFIED; free CLI that integrates with Claude Code/Cursor) now sells "custom pre-merge checks"; Cursor acquired Graphite (Dec 2025); Qodo 2.0 raised $70M explicitly on "code verification as AI coding scales"; SonarQube 2026.2 markets itself as "Fight AI Slop"; CodeScene published data that AI-generated changes fail 60%+ more often in unhealthy code and shipped a Code Health MCP server so agents can query health mid-session. Every one of these remains probabilistic or server-bound. None produces a local, deterministic, exit-code verdict against an executable spec.

**Architecture fitness functions** remain per-language and uneven: ArchUnit (Java) and dependency-cruiser (JS/TS, v18.1.0, very active) are solid wrap targets; PyTestArch is serviceable; **Rust and Swift have no established equivalent** (UNVERIFIED absence — nothing mature surfaced). The `craftsman arch` gate must therefore ship its own rule engine for exactly the two stacks the team cares most about — a real cost the fanout doc's design already anticipated, now confirmed as unavoidable.

**The decisive finding:** no product orchestrates static gates *plus* runtime gates (tests, axe-core a11y, Playwright visual, Lighthouse CI perf budgets, k6) under one CLI with unified exit codes and a shared baseline/ratchet. The market splits into static unifiers (qlty/trunk), siloed runtime CLIs, and generic runners with no gate semantics. AI reviewers are drifting toward "pre-merge checks" but remain probabilistic. **`craftsman check-all` targets an empty niche** — with the caveat that the niche may be empty because every project's runtime-gate wiring is bespoke; our per-stack opinionation is what makes it tractable. Note the entire "verification" market (Qodo's round, Sonar's positioning, CodeScene's research) is converging on Craftsman's founding thesis: deterministic checks on AI-written code. The thesis is validated; the mechanical-gate implementation is uncontested.

## Survey 4: Skill & Knowledge Products

- **Agent Skills standard** (agentskills.io, open-governed since Dec 2025): ~40-45 clients including Claude Code, Codex, Cursor, Gemini CLI, Copilot, Kiro, Amp, Factory. Skills are now the portable unit of methodology. Craftsman's agentskills.io conformance decision is confirmed correct.
- **Anthropic official skills** (162k stars): document/dev/design families — utilities, not a methodology.
- **Impeccable** (Paul Bakaus, 47.6k stars, Skill 3.9.1 Jul 2026): design language + 46 deterministic detector rules + 23 commands. The best evidence that "skill family + mechanical detectors" wins a domain. **Adopt for our frontend-design gate rather than compete.**
- **Trail of Bits skills**: the security-domain analogue. Candidate to wrap in `craftsman security` guidance.
- **skills.sh** (Vercel): cross-harness registry; install telemetry (929K claimed, self-reported UNVERIFIED). Distribution channel, not competitor; team-only audience means we can ignore it for now.
- **The methodology-family space has exactly two serious incumbents:** Superpowers (~257k stars) and **mattpocock/skills** (176k stars, v1.1.0 Jul 2026) — "Skills for Real Engineers." Pocock's family is a genuinely coherent methodology, not a grab-bag: user-invoked skills (`grill-with-docs`, `triage`, `to-spec`, `to-tickets`, `implement`, `improve-codebase-architecture`) plus model-invoked ones (`tdd`, `diagnosing-bugs`, `code-review`, `domain-modeling`), organized around four pillars — alignment via grilling before coding, shared domain language in CONTEXT.md, quality via TDD and systematic debugging, and anti-ball-of-mud architecture. Install bases are massive (grill-me 585K, tdd 463K per skills.sh telemetry — self-reported, UNVERIFIED). Everything else on the awesome lists is single-purpose tools or domain packs. So the space is effectively Superpowers (process-heavy: subagents, worktrees, written plans) versus Pocock (pragmatic, taste-driven) — and notably, **Pocock has a `to-spec` skill and Superpowers has a spec-compliance reviewer: both families are groping toward the spec-verification territory Craftsman claims**. Neither has mechanical verification, Gherkin, gate stacks, or git-ledger provenance; both have battle-tested skill *wording* at a scale a three-person team cannot replicate.
- **Interpretation for Craftsman:** the skills *format* war is over (the standard won), the skills *distribution* war is irrelevant (team-only), and the skills *content* war has two incumbents whose gap — no machine verdicts — is exactly our thesis. The correct posture is asymmetric: adopt their domain packs (Impeccable, Trail of Bits), steal their pedagogy (grilling, trigger-phrase discipline, token diets), and compete only on the verification spine.

## Survey 5: Agentic Dev Platforms (for completeness)

| Platform | State (2026-07) | Notable |
|---|---|---|
| Devin / Cognition | $25B valuation Apr 2026; Windsurf IDE rebranded Devin Desktop (Jun 2026); $20-$200/mo tiers | Reputation rehabilitated vs 2024-25; "89% of own code Devin-written" (company claim, UNVERIFIED) |
| Factory | $150M Series C at $1.5B (Apr 2026); Droids + multi-day "Missions" | Multi-model routing; **CLI supports Agent Skills** |
| Google Jules | Free (15 tasks/day) / Pro / Ultra (300/day, 60 concurrent) | Async VM-based; positioned for chores (bumps, migrations, test writing) |
| OpenAI Codex cloud | Parallel sandboxed tasks from web/CLI/GitHub/Linear/Slack | Multi-attempt comparison; skills-standard support |
| Codegen | **Absorbed into ClickUp**; standalone product gone | The platform-churn cautionary tale |

What platforms offer that methodology-on-harness cannot: fleet-scale parallelism (15-60 concurrent sandboxed tasks running hours or days), managed reproducible environments with org governance and audit trails, and non-IDE entry points (PM-initiated work from Linear/Slack). What they cannot offer: process quality — their output still depends on exactly the discipline a methodology encodes, which is why Factory, Codex, and Gemini CLI all *adopted* Agent Skills rather than replacing them. The equilibrium: **skills as the portable methodology layer, platforms as an execution substrate**. A skills-borne methodology also survives platform churn (see Codegen) in a way platform-native process configuration does not. Craftsman composes with these; it does not compete. Verdict: ignore as competitors, treat as future runtimes for `craftsman`-governed work.

## Cross-Cutting 2026 Themes

Four movements frame every verdict below:

1. **The SDD correction.** The 2025 spec wave met its critics in 2026 (review fatigue, waterfall regression, the sledgehammer problem), and every survivor responded with scale-adaptivity: BMAD V6's weight-adjusting workflows, OpenSpec's change deltas, Superpowers 6's token diet. A methodology that cannot scale its ceremony down dies in this climate.
2. **Specs-as-source retreated; skills-as-packages won.** Tessl pivoted its registry to skills; Amp shipped skills; BMAD adopted a skills architecture; every major harness reads SKILL.md. The unit of portable methodology in 2026 is the skill, not the platform.
3. **Verification became the market's word.** Qodo's $70M round, Sonar's "AI slop" positioning, CodeScene's failure data, Claude Code's review verification pass, Codex's `/goal` criteria — everyone now sells "verify AI code." Almost all of it is LLM-judged or server-bound. Deterministic local verification is the unclaimed corner of a validated market.
4. **The outer loop is forming.** Ronacher's "The Coming Loop" (queue → machine attempts → harness judges completion), Hashimoto's always-an-agent-running rule, Amp orbs, background-first subagents: the industry is building exactly the slot `craftsman verify` is designed to fill — the mechanical judge at the loop boundary.

## The Reuse-vs-Build Matrix

| Craftsman component | Closest existing thing | Verdict |
|---|---|---|
| Spec engine (Gherkin SPEC.md) | Kiro EARS specs; Spec Kit/OpenSpec prose specs; Knight's gherkin-guidelines.md | **Build ours** — no executable-spec incumbent; steal OpenSpec's specs/changes split, Knight's authoring rules |
| Verify adapters (6 stacks) | pytest-bdd, cucumber-js/playwright-bdd, cucumber-rs, bats — nothing unified; Swift path nonexistent | **Wrap runners, build the unification layer + Swift/bash code-gen** (per verification-cli doc) |
| Gates orchestration (`check-all`) | qlty (static), trunk check (architecture model), lefthook (execution) | **Build the orchestrator; wrap qlty-or-trunk for lint/security; wrap axe/lhci/Playwright per gate** — do not rebuild linter management |
| Baselines + ratchet | trunk hold-the-line; CodeScene trend gates; Betterer | **Steal the design**, own the unified snapshot |
| Docs pipeline (`craftsman docs`) | Context7, first-party MCPs, harness instruction files; Claude Code `/doctor` (first nibble) | **Build thin** — CLI-first cache is uncontested; MCPs as accelerators |
| Ledgers (git trailers) | Nothing — no harness or tool writes methodology provenance to git | **Build ours** (cheap, genuinely novel) |
| ADRs | MADR conventions, adr-tools | **Reuse conventions**, our consolidation layer only |
| Compaction | Claude Code auto memory + targeted summarize; Codex Memories | **Drop the session half; keep extraction-to-repo** of durable learnings only |
| Skills family | Superpowers, mattpocock/skills; house pattern (Fusion/Shaping Rooms) | **Build ours on the house pattern; adopt Impeccable + Trail of Bits for design/security domains; steal Superpowers' two-stage review and token diet** |
| Planning (PLAN.md batches) | Cursor plan files, ultraplan, Spec Kit tasks.md | **Build ours but integrate** — batches must feed native plan modes, not replace them |

## Differentiation Thesis, Stress-Tested

**Where Craftsman genuinely leads (specific, defensible):**

1. **Machine-verdict-only pass/fail.** Every competitor's spec-compliance check is an LLM opinion (Superpowers' review agent, Codex `/goal` verification, CodeRabbit checks). Craftsman's exit-code-only gate is the sole deterministic answer to "does the code match the spec" — and the one property that cannot be eroded by better models, only strengthened.
2. **The executable spec across six stacks including Swift.** Nobody runs Gherkin-as-agent-spec anywhere; nobody has a Swift Testing code-gen path. First-mover in an empty (if risky) niche.
3. **Unified static+runtime gate orchestration with baseline/ratchet.** Verified unoccupied. qlty stops at static; everything runtime is siloed.
4. **Git-ledger provenance.** No harness writes Learned:/Rejected:/Verified-by: trails into history. Cheap to build, compounding value, zero competition.
5. **Cross-harness consistency.** Same methodology on Claude Code, Codex, Cursor, Gemini CLI via the standard — the platforms themselves validated this by adopting skills.

**Where Craftsman would NOT outperform today (brutal):**

- **Planning/memory/compaction UX.** Ultraplan's commentable cloud plans, Cursor's parallel plan files, auto memory — all better than anything a PLAN.md convention plus CLI can deliver. If Craftsman fights here, it loses on every axis.
- **Skill wording maturity.** Superpowers has shipped six major versions of prompt-engineering refinement against millions of real sessions, including e2e tests of the methodology. Our first-draft skills will be objectively worse at triggering and pacing until dogfooded hard.
- **Small-task overhead.** The SDD critique applies to us: Gherkin + batches + gates on a one-line fix is Kiro's 16-acceptance-criteria bugfix all over again. Without a scale-adaptive light path, Craftsman is slower than a bare harness on the majority of everyday tasks — and "best-in-class efficiency" fails.
- **Step-definition economics.** The one documented experiment closest to our model concluded the glue-code overhead was questionable. Our code-gen approach (scenario = test function, no NL step matching) is the mitigation, but it is unproven at scale and highest-risk on Swift.
- **Bug-finding breadth.** CodeRabbit/Greptile-class reviewers catch cross-file semantic bugs that no mechanical gate will. Mechanical gates verify the spec; they do not review the code. We need AI review *in addition*, not instead.

**What must change to win:** a genuinely light default path (spec ceremony only when scope warrants — steal BMAD V6's scale-adaptive idea, of all things); token discipline matching Superpowers 6; integrate native plan mode and hooks as first-class citizens; prototype the Swift code-gen before betting the methodology on it.

## What Craftsman Dev Should Adopt

- **From Superpowers 4.x/6.x**: the two-stage review split (spec-compliance vs code-quality) — ours becomes mechanical-verify + LLM-quality-review; the token-diet discipline; e2e tests of the methodology itself.
- **From qlty/trunk**: wrap for lint/security gate internals; hold-the-line semantics for the ratchet.
- **From Impeccable**: adopt wholesale for the design domain (46 deterministic detectors align perfectly with machine-verdict philosophy); likewise Trail of Bits for security review guidance.
- **From Kiro**: event hooks concept → map onto harness-native hooks (Claude Code/Cursor), not a custom daemon.
- **From Cursor/Claude Code**: plans and memory are integration points — PLAN.md batches should be *produced through* native plan mode; extraction targets the repo, native memory keeps sessions.
- **From Andy Knight**: gherkin-guidelines.md as seed for the SPEC.md authoring skill.
- **From Willison/Ronacher**: "agentic engineering" vocabulary; the outer-loop framing (queue → attempt → machine judges completion) is exactly `craftsman verify` — cite it, ride it.

## What NOT to Adopt

- **BMAD's personas and ceremony** — rebuilding an agile team in prompts remains the antithesis of three-actor clarity, even in scale-adaptive V6.
- **Spec Kit's interlinked artifact chain** — the documented review-fatigue failure mode; our two artifacts (SPEC.md, PLAN.md) must stay two.
- **Tessl's spec-as-source** — the non-deterministic-compiler problem killed it; specs verify code, they don't replace it.
- **Custom session memory, compaction UX, checkpointing, orchestration runtimes** — harness-native, permanently better, actively evolving.
- **Marketplace/distribution machinery** — team-only audience; skills.sh telemetry races are noise for us.
- **LLM-judged gates in any form** — the moment a gate consults a model for pass/fail, the differentiation thesis collapses.

## Where We Would Lose Today

An honest scorecard, if Craftsman shipped as-designed against the July 2026 field:

1. **Against a bare Claude Code + Superpowers user on everyday tasks**: we lose on speed and token cost until the light path exists. Their brainstorm→TDD→review loop is fast, refined, and free of step-definition overhead.
2. **Against Kiro in a regulated enterprise**: we lose on native IDE integration, managed infrastructure, and procurement checkboxes. (We don't sell there; still, it's a loss.)
3. **Against mattpocock/skills on developer ergonomics**: grill-me/handoff/wayfinder are polished, zero-setup, and viral. Our CLI dependency is a real installation and trust hurdle even for experts.
4. **On Swift, today**: nothing exists to lose to — but our own code-gen path is unproven; until the prototype passes, the flagship stack is a liability, not an asset.
5. **On review quality**: any team running CodeRabbit/Greptile catches semantic bugs our gate stack never sees. Mechanical gates are necessary, not sufficient.

## The Bear Case

**Strongest form:** "Frontier harnesses absorb all of this within 12 months." The evidence is real: in one year Claude Code shipped plan mode → ultraplan, auto memory, checkpoints, dynamic workflows, and a code-review service *with a mechanical verification pass and CI-parsable severity output*. Codex shipped `/goal` with verification criteria. Kiro made specs native. Model-side improvement compounds: Willison now sometimes ships agent code unreviewed; if models stop needing scaffolding, process frameworks depreciate. Meanwhile two methodology families with six-figure star counts iterate weekly. Why build?

**The response:** Look at *what* was absorbed — session-side UX (plans, memory, undo, orchestration) — and what was not: durable project-side artifacts, opinionated gate policies, and git provenance. Harness vendors structurally avoid these: they are per-project opinions, cross-harness by nature, and liability-laden (a vendor won't declare your definition of "done"). Every absorption so far has *strengthened* the skills layer (all four majors adopted the standard Craftsman targets). Model improvement cuts the other way too: the better the model, the more code ships per hour, the more the binding constraint becomes *verification* — which is why Qodo raised $70M, Sonar rebranded around AI slop, and CodeScene published the failure data. Craftsman is deliberately positioned on the non-absorbable side: a thin, harness-integrated methodology whose only non-negotiable — the machine says pass/fail — becomes more valuable as everything else gets faster. And the team-only audience removes the one race we'd certainly lose (ecosystem scale). The bear case argues against building a Superpowers competitor or a session-UX layer. It does not argue against Craftsman — provided we build only the five uncontested components and integrate with everything else.

## Verification Notes

Claims in this document rest on four parallel research passes over official docs, changelogs, and repos on 2026-07-17. The following could **not** be verified against primary sources and are marked UNVERIFIED above: Kiro pricing/credit split and the Amazon Q shutdown date (third-party reporting only); Tessl Framework beta details (single third-party review); the Spec Kit-vs-OpenSpec 2x token benchmark; CodeRabbit ARR (Sacra estimate) and Greptile's 2026 round; all skills.sh/star-count/install telemetry (self-reported); absence of mature ArchUnit equivalents for Rust and Swift (searched, none found — absence, not proof); absence of any Trunk acquisition; Cursor's lack of checkpoint/rewind and auto-memory (absence in fetched docs); Devin's "89% of committed code" and Factory's user counts (company claims); exact Codex cloud metering as of July 2026. Adoption and revenue figures are the least reliable category everywhere; feature claims for the four major harnesses are the most reliable (all fetched from official docs).

Two absences carry design weight and deserve a re-check before 1.0: the empty Gherkin-as-agent-spec niche (a funded entrant could appear any quarter — Tessl's skills pivot shows how fast these companies reposition) and the empty static+runtime unified-gate niche (qlty is one plugin category away from claiming the runtime half).

## Key Sources

- Methodology: [obra/superpowers](https://github.com/obra/superpowers) + [Superpowers 4 announcement](https://blog.fsck.com/2025/12/18/superpowers-4/) · [github/spec-kit](https://github.com/github/spec-kit) · [Fission-AI/OpenSpec](https://github.com/Fission-AI/OpenSpec) · [bmad-code-org/BMAD-METHOD](https://github.com/bmad-code-org/BMAD-METHOD) · [Böckeler, martinfowler.com on SDD tools](https://martinfowler.com/articles/exploring-gen-ai/sdd-3-tools.html) · [Tessl launch](https://tessl.io/blog/tessl-launches-spec-driven-framework-and-registry/) · [Amp manual](https://ampcode.com/manual) and [news](https://ampcode.com/news) · [Aider-AI/aider](https://github.com/Aider-AI/aider) · [Willison on vibe coding vs agentic engineering](https://simonwillison.net/2026/May/6/vibe-coding-and-agentic-engineering/) · [Ronacher, "The Coming Loop"](https://lucumr.pocoo.org/2026/6/23/the-coming-loop/) · [Knight, BDD Gherkin guidelines for AI](https://automationpanda.com/2026/04/27/bdd-gherkin-guidelines-for-ai-coding-and-testing/)
- Harnesses: [Claude Code changelog](https://code.claude.com/docs/en/changelog), [memory](https://code.claude.com/docs/en/memory), [checkpointing](https://code.claude.com/docs/en/checkpointing), [workflows](https://code.claude.com/docs/en/workflows), [code review](https://code.claude.com/docs/en/code-review) · [Codex changelog](https://developers.openai.com/codex/changelog), [AGENTS.md](https://developers.openai.com/codex/agent-configuration/agents-md), [skills](https://developers.openai.com/codex/skills/) · [Cursor changelog](https://cursor.com/changelog), [hooks](https://cursor.com/docs/agent/hooks), [planning](https://cursor.com/docs/agent/planning) · [Gemini CLI docs](https://geminicli.com/docs/) · [agents.md](https://agents.md) · [agentskills.io](https://agentskills.io)
- Gates and review: [trunk-io/plugins](https://github.com/trunk-io/plugins) · [qltysh/qlty](https://github.com/qltysh/qlty) · [oxsecurity/megalinter](https://github.com/oxsecurity/megalinter) · [evilmartians/lefthook](https://github.com/evilmartians/lefthook/releases) · [CodeRabbit CLI](https://www.coderabbit.ai/blog/coderabbit-cli-free-ai-code-reviews-in-your-cli) · [Cursor × Graphite](https://cursor.com/blog/graphite) · [SonarQube 2026.2](https://www.sonarsource.com/products/sonarqube/whats-new/2026-2/) · [CodeScene Code Health MCP](https://codescene.com/product/code-health-mcp) · [Qodo Series B](https://techcrunch.com/2026/03/30/qodo-bets-on-code-verification-as-ai-coding-scales-raises-70m/) · [dependency-cruiser](https://github.com/sverweij/dependency-cruiser)
- Skills and platforms: [anthropics/skills](https://github.com/anthropics/skills) · [pbakaus/impeccable](https://github.com/pbakaus/impeccable) · [mattpocock/skills](https://github.com/mattpocock/skills) · [trailofbits/skills](https://github.com/trailofbits/skills) · [skills.sh](https://www.skills.sh/) · [block/agent-skills](https://github.com/block/agent-skills) · [devin.ai](https://devin.ai/pricing) · [factory.ai](https://factory.ai/) · [jules.google](https://jules.google/) · [codegen.com](https://codegen.com/)

## Conclusion

The wheel-reinvention audit passes, with conditions. Five components target verified-empty niches: the executable Gherkin spec engine, the cross-stack verify adapters, the static+runtime gate orchestrator with ratchet, the git ledger, and the CLI-first docs pipeline. Three components must be reduced to integration or convention: compaction (repo-extraction only), planning (feed native plan modes), ADRs (adopt MADR). One component — the skills family — enters a two-incumbent market and wins only by carrying the mechanical-verification differentiator those incumbents lack, while stealing their hard-won token discipline. The honest risks are the light-path gap on small tasks, unproven Swift code-gen, and first-draft skill maturity. The bear case is strong against session-UX ambitions and weak against the actual design. Build the five, wrap the rest, dogfood ruthlessly, and re-run this audit before 1.0.
