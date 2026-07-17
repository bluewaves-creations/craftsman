# Production-Grade Agent Output: The Evidence Base

> What measurably improves agent code quality in 2025–2026, how to encode modern clean architecture per stack, and how to prove Craftsman Dev outperforms — with evidence strength graded throughout.

---

## The Question

Craftsman Dev bets that mechanical gates plus human-owned specs produce production-grade code from agents. Is that bet supported by evidence? Which interventions have measured effects, which are folklore, and what exactly should the per-stack quality skills contain versus what tooling should enforce?

Evidence grading used below: **Strong** (peer-reviewed or large-N with controls), **Moderate** (benchmark studies, vendor evals with methodology, replicated field data), **Weak** (small-N, single-source), **Anecdotal** (practitioner reports, vendor marketing). Anything I could not verify directly is marked ⚠️.

## Part 1 — What Measurably Works

### The benchmark landscape, mid-2026 (context for every claim below)

- **SWE-bench Verified is saturated**: frontier models cluster near 88%, and OpenAI publicly abandoned it in February 2026. METR's analysis found many SWE-bench-passing PRs would not survive human code review — the benchmark overweights quick fixes and underweights architectural reasoning. **Moderate.**
- **SWE-bench Pro and Terminal-Bench 2.0** are where differentiation lives now. Terminal-Bench 2.0 (ICLR 2026): top harnesses at 88–92% (89 tasks, 16 categories, Docker-isolated, exit-code verified — structurally similar to Craftsman's Machine actor). **Moderate.**
- **METR time horizons**: 50%-success task length now improving ~10x/year (doubling ~4.3 months post-2023, vs 7 months 2019–2023); the Jan 2026 v1.1 suite added long tasks and notes measurements above 16 hours are unreliable. Long-horizon reliability — not raw capability — is the binding constraint. **Moderate.**

Implication: capability is not the bottleneck for a small expert team in 2026. Process and verification are.

### Process interventions with measured effects

**Spec-first / TDD with agents — the mechanism matters more than the ritual.** The TDAD study (arXiv 2603.17973) is the sharpest result of the year: giving agents a *dependency map between source and tests* (pre-change impact analysis) cut regressions by 70% (6.08% → 1.82%) and lifted resolution 24% → 32%. Critically, **procedural TDD instruction alone made things worse** (regressions rose to 9.94%). Separately, "Rethinking the Value of Agent-Generated Tests" (arXiv 2602.07900, six models on SWE-bench Verified) found that the volume of agent-written tests does not change outcomes — agents use their own tests as debugging printf, not verification (print statements outnumber assertions). **Strong.**

Reading for Craftsman: the win is not "tell the agent to do TDD." The win is (a) tests the agent didn't write and can't redefine (human-gated Gherkin SPEC.md), and (b) mechanical infrastructure that tells the agent which scenarios its change can break. This validates the three-actor split precisely: procedural instructions to the Agent underperform mechanical context from the Machine.

**LLM-as-judge is not a gate.** LLMs verifying code against natural-language specs recognize correctness at only 52–78% (arXiv 2508.12358); adding reasoning steps *increased* misjudgment; RAND's 2026 Judge Reliability Harness found no judge uniformly reliable; and LLM-generated verifiers show tightly correlated errors (a homogenization trap — the judge misses what the generator missed). **Strong.** Mechanical checks (exit codes) remain the only trustworthy verdict. LLM review is a useful *advisory* signal for taste, never a pass/fail gate.

**DORA 2025 (State of AI-assisted Software Development, N≈5,000):** AI is an amplifier — adoption now clearly correlates with higher throughput *and* higher instability (more change failures, more rework). Time saved in generation is re-spent on auditing. Returns come from the surrounding system (fast feedback, small batches, strong version control hygiene), not the tool. **Moderate-Strong** (survey-correlational). This is the empirical case for the gate stack: throughput without a stability mechanism is negative-sum.

**Context engineering has measured effects.** Anthropic's evals: context editing alone +29% on agentic tasks, +39% with a memory tool, and 84% token reduction on a 100-turn eval; subagent isolation with condensed 1–2K-token returns outperformed single-agent by ~90% on multi-domain research tasks, and token usage explained ~80% of performance variance on BrowseComp. **Moderate** (vendor evals, but consistent across sources). Craftsman's compaction-by-extraction and subagent gates are directionally confirmed.

**AGENTS.md — the surprise result.** ETH Zurich (Gloaguen et al., arXiv 2601.20404; 138 real repos): LLM-generated context files *reduced* task success ~3% and increased inference cost >20%; developer-written files helped only marginally (~+4%) and **only when minimal and precise**. Agents follow verbose instructions faithfully — which broadens exploration and burns tokens without improving outcomes. **Strong.** Consequence: AGENTS.md must stay short, human-written, and contain only what tooling cannot enforce; every rule that can become a mechanical gate should be one.

**Long-horizon degradation is real and measured.** SlopCodeBench (arXiv 2603.24755; 11 models, 20 iterative problems): verbosity rises in 90% of agent trajectories and architectural erosion in ~80%, versus 55% of human repos; median code growth 43% vs 25% for humans. "Agentic entropy" (arXiv 2604.16323): agents optimize local correctness while drifting from global design because context windows hide systemic intent. **Moderate-Strong.** Countermeasures with evidence: complexity/duplication/size thresholds as blocking gates, small diffs, and forced reuse checks — i.e., ratcheted fitness functions, not exhortations.

### Quality-gate evidence

**Mutation testing earns a gate — diff-scoped.** AI-generated test suites achieve high coverage with dismal fault detection: one 2026 analysis measured a 57.3% mutant kill rate (43% of simple seeded bugs undetected), and mutant survival runs 15–25% higher on AI-generated code *at equivalent coverage*. Coverage is gameable by an agent (execute lines, assert nothing); mutation score is structurally harder to game. Meta runs LLM-driven mutation testing in production (73% of generated tests accepted). Feasibility: full-run mutation is too slow as a gate, but **differential mutation on changed files runs in minutes** — mutmut (Python), Stryker (TS), cargo-mutants (Rust) are all mature; Swift has no first-class tool (muter exists but is not production-consensus ⚠️). **Moderate-Strong.** Verdict: add `craftsman verify --mutate` as a diff-scoped, ratcheted gate on Python/TS/Rust; skip for Swift/bash initially.

**Code health metrics are valid but vendor-adjacent.** CodeScene's "Code Red" study (arXiv 2203.04374; 39 production codebases): low-health code takes >2x longer to change and carries ~15x more defects; r = −0.58 between health and time-in-development; follow-up work found Code Health aligns with human maintainability judgment better than competing metrics, and a 2026 paper extends it to "AI-friendliness" (arXiv 2601.02200). Caveat: the strongest studies are CodeScene-affiliated, though peer-reviewed. **Moderate.** A CodeScene-style health score (function length, nesting, cohesion, duplication) is a defensible numeric gate — implement open-source, don't buy the claim wholesale.

**Property-based testing**: the strongest published evidence remains Goldstein et al., "Property-Based Testing in Practice" (ICSE 2024, Jane Street) — practitioners find PBT effective for stateful/parsing/roundtrip code but adoption is bounded by property-writing skill. ⚠️ I could not verify 2026-specific agent+PBT results in this pass. **Weak-Moderate.** Sensible position: PBT as a recommended pattern in skills (roundtrip, invariant, oracle properties) — not a universal gate.

**Types and linters**: long-established defect-reduction evidence (e.g., the classic ~15% of JS bugs preventable by TS study) and, more importantly for agents, deterministic feedback loops are "the one form of self-correction that reliably works." Strict typing is the cheapest agent-gaming-resistant gate available. **Strong** (by accumulation).

### Where agents fail in 2026 field reports

- **Security**: Veracode Spring 2026 — ~45% of unprompted AI samples fail security tests; XSS 86% fail, log injection 88% fail, Java worst at 72%; models cannot do the interprocedural taint reasoning these require. CSA documents an AI-generated CVE surge. **Strong.** Countermeasure with evidence: SAST/taint gates (semgrep, CodeQL) — the failures are exactly the mechanically detectable classes.
- **Erosion/duplication/verbosity**: see SlopCodeBench above — regenerate-rather-than-reuse is the signature agent failure; duplication detection as a blocking gate is the countermeasure.
- **Premature completion & hollow tests**: covered in the adaptive-planning research doc; mutation testing is the anti-hollow-test gate.
- **Over-engineering**: agents add speculative abstraction under vague instruction; minimal-precise specs (ETH result) and diff-size budgets counter it. **Moderate.**

### Efficiency evidence

Subagent isolation and compaction numbers above are the best-measured levers (84% token reduction; isolated subagents ~9K vs ~15K tokens for accumulating patterns; token usage explaining ~80% of performance variance on browsing tasks). Two practical corollaries:

- **Extraction beats summarization.** Anthropic's context-editing evals show removing stale tool results (+29%) outperforms naive rolling summaries; Craftsman's "compaction by extraction" (keep decisions, spec state, and ledger; drop transcripts) matches the winning pattern.
- **Failure loops are the token sink.** DORA 2025 and the agent-test-value study both show verification/rework consuming the savings from generation. Tokens-per-green-scenario (Part 3) makes this visible; a methodology that fails gates less often is cheaper even at higher per-turn cost.

Model choice: vendor guidance and practitioner consensus say small models (Haiku-class) suffice for mechanical steps — gate orchestration, log triage, commit-trailer extraction, docs-pipeline chunking — while frontier models are reserved for implementation and SPEC.md drafting. ⚠️ I found no rigorous 2026 study quantifying quality loss from small-model gate-running; treat as **Anecdotal-Weak** but low-risk, since gates are exit-code-verified anyway (a wrong small-model summary can't flip a gate). The reverse claim — that frontier models are *required* for spec drafting — is also unmeasured; worth testing in the internal eval.

### Part 1 evidence summary

| Intervention | Measured effect | Strength |
|---|---|---|
| Impact-mapped test context (TDAD) | −70% regressions; +8pt resolution | Strong |
| Procedural "do TDD" instruction | Regressions *worsened* (6.08% → 9.94%) | Strong |
| Agent-written test volume | No effect on outcomes; cost only | Strong |
| LLM-as-judge for correctness | 52–78% recognition; correlated errors | Strong |
| Minimal human AGENTS.md | ~+4% success | Strong |
| Generated/verbose context files | −3% success, +20% cost | Strong |
| Strict types + deterministic linters | Reliable self-correction signal | Strong (accumulated) |
| Diff-scoped mutation gate | Exposes 15–25% weaker AI tests at equal coverage | Moderate-Strong |
| Complexity/duplication fitness gates | Counters measured 80–90% erosion/verbosity trajectories | Moderate |
| Code health score (CodeScene-style) | 2x change time, 15x defects in low-health code | Moderate |
| Context editing / subagent isolation | +29–39% task perf; −84% tokens | Moderate (vendor) |
| SAST gates for XSS/log-injection/taint | Targets 85%+ unprompted failure classes | Moderate (inference from strong failure data) |
| Property-based testing | Effective for invariant-heavy code; skill-bounded | Weak-Moderate ⚠️ |
| Small models for mechanical steps | Unquantified; structurally low-risk | Anecdotal |

## Part 2 — Encoding Modern Clean Architecture Per Stack

The governing principle from Part 1: **prose rules decay, gates don't.** Skills should teach idiom and architecture *shape*; everything expressible as a check belongs in tooling. Per stack:

### Swift / Apple

| Dimension | 2026 consensus |
|---|---|
| Authoritative sources | Swift API Design Guidelines (swift.org); Swift 6.2 migration guide; Apple's SwiftUI + Observation docs; WWDC 25/26 sessions |
| Concurrency | Swift 6.2+ Approachable Concurrency: `SWIFT_APPROACHABLE_CONCURRENCY=YES`, **default MainActor isolation** (SE-0466), `nonisolated async` runs on caller's actor (SE-0461). Rule: stay on main; escape with `@concurrent` only when measured |
| Architecture | ⚠️ Not fully verified this pass: consensus reading is vanilla **MV with @Observable models + service/client layer** for new apps; TCA remains a deliberate opt-in for teams wanting exhaustive testability, not the default. MVVM-as-ceremony is fading |
| Tooling enforces | swift-format (formatting, Xcode-integrated); SwiftLint (semantic rules: force_unwrap, cyclomatic complexity, file length); strict concurrency = the compiler is the race gate |
| Skill teaches | API Design Guidelines idiom (clarity at point of use), value types first, @Observable patterns, when to leave MainActor, dependency injection without frameworks |

Division of labor: Swift is the stack where the *compiler* is the strongest gate — strict concurrency in 6.2+ turns data-race safety from a review topic into an exit code. The skill's highest-value content is therefore the parts the compiler can't see: API naming per the Design Guidelines (agents trained on a decade of Objective-C-flavored Swift drift here), when `@concurrent` is justified, and keeping views free of business logic. SwiftLint should carry the mechanical remainder (complexity, force-unwrap, file length) with Craftsman's ratchet on brownfield. Mutation testing is the one gap in the Swift gate stack (⚠️ no production-consensus tool).

### Python

| Dimension | 2026 consensus |
|---|---|
| Authoritative sources | typing.python.org docs; uv docs; PEP 8 + PEP 621; Astral's toolchain docs |
| Toolchain | **uv** (packaging/venv/lock — poetry/pip displaced); **ruff** (lint+format, 800+ rules); **pyright** (strict) — Astral's `ty` is emerging but pyright remains the verified floor mid-2026 |
| Architecture | src-layout, `pyproject.toml` as single config; typed-first (3.13/3.14: no `from __future__`); pydantic v2 at I/O boundaries only — plain dataclasses/protocols internally |
| Tooling enforces | ruff (incl. bandit rules), pyright strict, pytest, mutmut (diff-scoped), uv lock |
| Skill teaches | Protocol-based ports, boundary validation pattern, exception design (narrow, typed), when pydantic vs dataclass, no God-modules |

Division of labor: Python's 2026 convergence is unusually clean — uv + ruff + pyright strict is a genuine floor, all configured in one `pyproject.toml`, and agents comply with it well because the feedback is instant and deterministic. The skill's job is the parts type checkers can't express: keeping pydantic at the edges (agents love to thread `BaseModel` through domain logic), Protocol-typed ports so Gherkin scenarios run against fakes without mock frameworks, and the "Hypermodern Python" successor stance that the packaging wars are over — no requirements.txt, no setup.py, no poetry in new code.

### TypeScript

| Dimension | 2026 consensus |
|---|---|
| Authoritative sources | TS handbook strictness flags; Zod v4 docs; Node ESM docs; Biome docs |
| Toolchain | **Biome default for new projects** (10–25x faster, lint+format in one); ESLint flat-config survives where framework plugins (react-hooks) matter; **ESM-only** for new code — dual CJS/ESM is a documented minefield |
| Architecture | `strict: true` plus the extra flags (`noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`); **Zod v4** at boundaries (14x faster parse, 5.4KB); monorepo via pnpm/turborepo workspaces. **effect-ts: growing but viral** — once one function returns Effect, it owns your control flow; not a house default |
| Tooling enforces | tsc strict, Biome, Stryker (diff-scoped), knip (dead code) |
| Skill teaches | Parse-don't-validate at boundaries, discriminated unions over class hierarchies, Result-style error returns where it pays, no `any`/assertion escapes |

Division of labor: TypeScript is the stack most exposed to agent gaming — `as any`, non-null assertions, and `@ts-ignore` are one-token escapes from every guarantee. The gate must therefore ban the escapes mechanically (Biome/ESLint rules, `tsc` with the full strict-flag set), and the skill teaches the constructive alternative: parse at the boundary with Zod v4, model states as discriminated unions, and let inference flow. On the framework question: Biome for new projects, ESLint where react-hooks-class plugins are load-bearing; effect-ts explicitly *not* house style (its own advocates concede it "wants to own your control flow") — a per-project ADR decision.

### Rust

| Dimension | 2026 consensus |
|---|---|
| Authoritative sources | Rust API Guidelines (rust-lang.github.io/api-guidelines); std docs; clippy book |
| Errors | Libraries: `thiserror` enum per crate, `#[from]` wrapping. Binaries: `anyhow::Result` + `.with_context()` at I/O boundaries. No `unwrap` outside tests |
| Architecture | Workspace with focused crates; `[workspace.lints]` shared; `clippy::pedantic` + `nursery` as **warn** at root, `-D warnings` in CI, narrow scoped `#[allow]` with reasons only |
| Unsafe | `#![deny(unsafe_op_in_unsafe_fn)]`; deny unsafe at root by default; every block carries `// SAFETY:`, every unsafe fn a `# Safety` section |
| Skill teaches | Ownership in public APIs (take `&str`, return owned), typestate where it clarifies, when to split crates, async runtime discipline (no lock across await) |

Division of labor: Rust's gate stack is the most complete of any Craftsman stack — clippy pedantic/nursery plus `-D warnings` plus cargo-mutants covers most of what a reviewer would say. The observed agent failure mode is writing "C++ in disguise": clone-everything ownership, `unwrap` chains, stringly-typed errors. The skill teaches the API Guidelines checklist (C-CALLER-CONTROL, C-GOOD-ERR et al.) and the thiserror-for-libraries / anyhow-for-binaries split; the workspace `[workspace.lints]` table makes the policy inherit mechanically across crates so it cannot drift per-member.

### bash

| Dimension | 2026 consensus |
|---|---|
| Authoritative sources | Google Shell Style Guide; shellcheck wiki |
| Tooling enforces | shellcheck (blocking), shfmt (formatting), `set -euo pipefail` |
| Graduation rule | Google's own rule: beyond ~100 lines, or needing arrays/nontrivial control flow/testing, **rewrite in a real language** (for Craftsman: Rust or Python). Skills should make this a hard checkpoint, not advice |
| Skill teaches | Quoting discipline, trap-based cleanup, no parsing `ls`, prefer `$(...)`, when a script is actually a CLI feature request |

Division of labor: bash is where agents produce plausible-looking code with the highest latent-failure density (unquoted expansions, swallowed exit codes in pipes). shellcheck as a blocking gate catches the majority mechanically; the skill's single most valuable rule is the graduation checkpoint — Google's guide already says it, and for Craftsman it should be enforced at review: a shell script crossing ~100 lines or growing argument parsing is a `craftsman` CLI feature or a Python script, full stop.

### Cross-stack: what architecture guidance actually helps agents

Direct evidence is thin (**Weak-Moderate**, mostly inference from strong adjacent results), but three findings converge:

1. **Locality wins.** Agents fail systemically when correctness requires context outside the window (agentic entropy result). Vertical-slice / feature-folder organization keeps everything a change needs in one place — it is the architecture that matches how agents read code. Layered/hexagonal ceremony that scatters one feature across five directories actively fights the tool.
2. **Ports at real boundaries only.** Hexagonal thinking earns its cost exactly where Craftsman needs seams for testing (Machine actor): I/O, external services, clock/randomness. As a global pattern it is 2015 dogma; as a boundary pattern it is what makes Gherkin scenarios executable without mocks everywhere.
3. **Architecture rules belong in fitness functions, not prose.** The ETH AGENTS.md result generalizes: verbose architecture prose in context files is net-negative; a dependency-direction check (`craftsman arch`) that fails the build is followed 100% of the time. Encode "domain does not import infrastructure," module size caps, and duplication ceilings as gates; keep AGENTS.md to taste and vision.

On bounded contexts: for a small expert team, DDD's strategic patterns matter more than its tactical ones. A bounded context maps naturally to "what fits in one agent session with one SPEC.md" — a unit of context, in both the DDD and the LLM sense. The skill-level guidance that survives the evidence: one context per deployable or per feature area, explicit published interfaces between contexts (which the arch gate can check as import rules), and no shared mutable "common" module — the place agentic duplication-avoidance goes to die, because agents dump everything there. Everything more elaborate (aggregate ceremony, repository-per-entity, CQRS by default) is 2015 dogma that inflates diffs — the measured agent failure direction — without a corresponding gate to hold it in place.

How this maps to Craftsman's existing gate stack:

| Craftsman gate | Evidence it rests on | Grade |
|---|---|---|
| verify (Gherkin, exit codes) | TDAD; LLM-judge failure data; premature-completion research | Strong |
| lint / types | Deterministic-feedback self-correction; strict-typing defect evidence | Strong |
| arch (fitness functions) | SlopCodeBench erosion; agentic entropy; ETH prose-vs-gate result | Moderate-Strong |
| security | Veracode/CSA failure classes are mechanically detectable | Strong |
| health (CodeScene-style) | Code Red 2x/15x; maintainability-judgment alignment | Moderate |
| mutation (proposed addition) | AI-test hollowness data; differential feasibility | Moderate-Strong |
| perf / a11y / visual | Standard tooling; no agent-specific 2026 evidence found ⚠️ | Weak (as agent gates) |

## Part 3 — Measuring "Outperform"

Public benchmarks cannot compare *methodologies*, for three structural reasons:

1. **Saturation and contamination.** SWE-bench Verified is clustered at ~88% and abandoned by its refiner; its tasks are in every frontier model's training distribution. Any methodology delta would be noise at the ceiling.
2. **Task-shape bias.** METR showed SWE-bench overweights short, localized fixes — exactly the regime where methodology matters least. Craftsman's value proposition (long-horizon, multi-session, production-grade) is the regime METR says current suites measure unreliably (>16h).
3. **Confound of harness and process.** Terminal-Bench leaderboard entries vary model *and* harness *and* prompting simultaneously; nothing isolates the process variable Craftsman needs to defend.

So the comparison must be internal: model held constant, process varied, verdicts mechanical. DORA 2025 supplies the outcome frame a small agentic team should report — throughput (lead time per shipped scenario) is meaningless without stability (change failure rate, rework rate), and the 2025 report's core finding is precisely that AI moves the first without the second unless the surrounding system compensates. Add two Craftsman-native metrics:

- **Tokens-per-green-scenario**: total session tokens ÷ scenarios green at final gate-pass. Prices failure loops, verbosity, and compaction quality in one number, and is immune to "looks done" inflation because green is mechanically defined.
- **Gate-recidivism rate**: fraction of gate failures that recur on the same gate within a session. High recidivism means the agent is thrashing, not learning from the deterministic signal — an early indicator that a skill or gate message needs rewording.

## What Craftsman Dev Should Adopt

1. **Mechanical test context, not TDD sermons** — build TDAD-style pre-change impact mapping (which scenarios/tests can this diff break) into `craftsman verify`; drop any purely procedural "do TDD" language from skills (evidence says it backfires).
2. **Diff-scoped mutation testing as a ratcheted gate** for Python (mutmut), TypeScript (Stryker), Rust (cargo-mutants); mutation score on changed lines, minutes not hours. Coverage stays as a floor, never a target.
3. **Security gate weighted to the measured failure classes** — semgrep/CodeQL rules for XSS, log injection, taint flows; these are exactly where models fail 85%+ unprompted.
4. **Fitness functions against agentic entropy** — duplication ceiling, function/file size caps, complexity thresholds, dependency-direction rules, all ratcheted from baseline for brownfield.
5. **Minimal, human-written AGENTS.md** — a hard length budget (≪ 100 lines of rules); everything mechanizable moves to gates. Never auto-generate it.
6. **Vertical-slice organization with ports at I/O boundaries** as the cross-stack architecture default in skills.
7. **Compaction + subagent isolation** as first-class: gates run in subagents returning verdict + minimal failure context, not transcripts.
8. **Small models for mechanical steps** — safe because gates are exit-code-truthed; measure, don't assume.

## What NOT to Adopt

- **LLM-as-judge as a pass/fail gate** — 52–78% correctness recognition, correlated errors, no reliable judge. Advisory review only.
- **Coverage percentage targets** — trivially gamed by agents; mutation score supersedes it.
- **Verbose architecture prose in context files** — measured net-negative (−3% success, +20% cost when generated; only minimal human files help).
- **Agent-authored tests as evidence of quality** — volume doesn't move outcomes; only human-gated SPEC.md scenarios count toward done.
- **Full-codebase mutation runs in the loop** — too slow; diff-scoped only.
- **effect-ts as house style; TCA as Swift default; global hexagonal layering** — heavy frameworks whose viral cost outweighs benefit for a small team; opt-in per project with an ADR.
- **Public-benchmark bragging rights as the success metric** — saturated and unrepresentative of production-grade.

## Conclusion

The 2025–2026 evidence base converges on one sentence: **agents improve where feedback is mechanical, contextual, and impossible to argue with — and degrade where quality depends on instructions, self-assessment, or judgment they can reinterpret.** Every strong result (TDAD's 70% regression cut, the LLM-judge failure numbers, DORA's throughput/instability tension, the AGENTS.md minimalism result, SlopCodeBench's erosion curves) points the same direction Craftsman Dev already faces. The refinements are: impact-mapped verification, mutation over coverage, security gates targeting measured failure classes, entropy fitness functions, and ruthless brevity in prose context.

### Proposed eval protocol (runnable by a 2–4 person team in ~a week)

1. **Task set**: 8 tasks × the team's real stacks — 4 greenfield features, 2 brownfield changes in a seeded repo, 1 bugfix, 1 refactor. Each with a hidden human-written acceptance suite (Gherkin) the agent never sees in conditions B/C. Human-estimated size: 1–4 hours each.
2. **Conditions** (same model, same harness, N=3 runs each): **A** = Craftsman Dev full stack (SPEC.md + gates); **B** = plain agent + "write tests, follow best practices" prompt; **C** = agent + minimal AGENTS.md, no gates.
3. **Judges are mechanical only**: hidden acceptance suite pass rate; gate outcomes (lint/arch/security/health) run identically on all conditions post-hoc; diff-scoped mutation score; duplication and complexity deltas.
4. **Metrics**: green-scenario rate (primary), change failure rate (does run N break run N−1's scenarios in brownfield tasks), rework rate (fraction of diff rewritten within the session), tokens-per-green-scenario, wall-clock lead time.
5. **Analysis**: paired per-task comparison, report medians with all runs published; a methodology "wins" only if it beats both alternatives on green-scenario rate *without* losing on tokens-per-green-scenario by >2x.
6. **Ratchet**: re-run quarterly with the current default model; the task set is versioned, and any task a condition saturates (9/9 green) is replaced with a harder sibling.

That protocol is small enough to actually run, mechanical enough that no one can argue with the verdict, and DORA-aligned enough to translate into language the rest of the industry already speaks.

---

*Verification notes: SwiftUI MV-vs-TCA consensus and property-based-testing efficacy could not be fully verified within this pass (search budget) and are marked ⚠️; model-choice-for-gates guidance is vendor/practitioner anecdote. All arXiv identifiers and quantitative claims otherwise trace to the sources gathered July 2026: TDAD (2603.17973), agent-test value (2602.07900), LLM-judge failures (2508.12358), AGENTS.md efficiency (2601.20404), SlopCodeBench (2603.24755), agentic entropy (2604.16323), Code Red (2203.04374), DORA 2025 (dora.dev/dora-report-2025), Veracode Spring 2026 GenAI Code Security Update, METR time-horizons v1.1, Terminal-Bench 2.0 (tbench.ai).*
