# Craftsman Skill Family — Decomposition Design

> Six workflow skills, gear-routed, sharing one conventions file and one CLI. The design phase's first artifact, derived from the 22-document research corpus and approved decisions of 2026-07-17.

---

## Design inputs

- **House pattern** (agent-agnostic-skills research): one skill = one accountability; gear routing inside skills; byte-identical `references/craftsman-conventions.md` in every skill; five-move description formula; ~500-char description budget; SKILL.md < 150 lines with per-gear references; single-writer rule (only the CLI touches generated state).
- **Design mandates** (README, validation research): scale-adaptive light path; minimal human-written AGENTS.md; no TDD sermons — mechanical context instead; integrate harness natives (plan mode, memory), don't compete; AI review advisory, never a gate.
- **Approved forks** (2026-07-17): 6-skill family · stack idiom as references in implement · light path as a gear + conventions routing rule · brownfield as an init gear.

## The family at a glance

| Skill | Accountability | Gears (\* = safe default) | Destructive gears |
|---|---|---|---|
| `craftsman-init` | Bring a repo under the methodology | `new` · `adopt` · `upgrade` | all three (no default — ask) |
| `craftsman-spec` | The executable specification | `draft`\* · `delta` · `recover` | none (SPEC.md writes are human-approved) |
| `craftsman-plan` | Batching and replanning | `batch`\* · `revise` · `gap` | none |
| `craftsman-implement` | Turning red scenarios green | `batch`\* · `boundary` · `finish` · `quick` | none (commits are the CLI's, gated) |
| `craftsman-fix` | Root-cause bug fixing | `diagnose`\* · `fix` · `improve` | none |
| `craftsman-review` | Advisory judgment | `quality`\* · `design` | none |

Six catalog entries ≈ 3.2k chars of descriptions — comfortably inside the ~8k (Codex) and ~16k (Claude Code) catalog budgets even alongside a user's other skills.

## Shared architecture

**The conventions file.** Every skill carries a byte-identical `references/craftsman-conventions.md` (read once per session), containing exactly what every gear needs and nothing stack- or gear-specific:

1. The three-actor rules (human owns vision + SPEC.md approval; agent is the librarian; machine verdicts are exit codes — never ask an LLM whether code works).
2. The single-writer rule: PLAN.md state markers, ledger trailers, baselines, session extracts, and the docs cache are written through `craftsman` commands only. Skills judge; the CLI records.
3. The ledger vocabulary: commit types (`feat(batch-N)`, `fix`, `refactor`, `retro-spec`, `chore(deps)`) and trailers (`Scenarios:`, `Verified-by:`, `Learned:`, `Rejected:`, `Ref:`, plus `Dependency:` for new deps).
4. The gate table: `craftsman verify | lint | arch | security | health | mutate | perf | a11y | visual`, their modes (`off | baseline | strict`), and the rule that a red gate blocks the boundary.
5. **Scale routing** (the light path): bugfix → `craftsman-fix`; small scoped change with no new externally visible behavior → `craftsman-implement` `quick`; new behavior → `craftsman-spec` first. `quick` skips SPEC/PLAN ceremony but never skips gates or the `Verified-by:` trailer.
6. The doc-grounding rule: no source, no code — consult the AGENTS.md Documentation Sources table via `craftsman docs`; unlisted library → stop and ask.
7. The destructive-gear rule: destructive gears are never reached by near-miss inference; confirm by name before writing.
8. The recovery budget: three classified attempts (implementation), one (environment), then stop, report, draft ADR.
9. The pre-action gate: before proposing any architectural approach, read `decisions/index.md` and query `git log --grep="Rejected:"` for the touched area; a match means warn and confirm, never silently retry a rejected approach.
10. Fan-out discipline: research runs in isolated subagents that return findings, not transcripts; fan-out is reserved for research isolation, best-of-N attempts, and independent batches in worktrees — never for sequential work or review.

**Gear routing.** Each SKILL.md follows the Fusion skeleton: identity paragraph → "read `references/craftsman-conventions.md` once per session" → a routing table (`Signal | Gear | Load references/<gear>.md`) → gear-specific rules → a closing `## Never` list. Gear content lives in `references/<gear>.md`, loaded only when that gear fires.

**Descriptions.** Five moves, in order: identity → gears with safe default → quoted literal triggers → cross-routing → applicability gate. Third person, trigger keywords in the first 50 chars, ≤ 600 chars each. Every description ends with the same gate: *"Applies only inside a Craftsman project (craftsman.toml present); otherwise offer `craftsman init` and stop."* — the family-level mutual-exclusion gate from the Shaping Rooms pattern.

## The six skills

### 1. craftsman-init — bootstrap

**Gears** (all destructive; no default — ambiguous request → ask):

- `new` (greenfield): interview the human for a **minimal** AGENTS.md — hard length budget enforced, observed facts and taste only, never generated architecture prose (ETH result) — including the Documentation Sources table (library → source → pinned → verify gate); write `craftsman.toml` (stack auto-detected, human-confirmed); wire the verify harness for the stack; on Apple, probe `xcrun agent skills export` and install Apple's skills alongside; run `craftsman doctor` to prove the loop closes (one trivial scenario red → green).
- `adopt` (brownfield): drive the five-phase protocol — Phase 0 observe (read-only research doc with confidence labels) → Phase 1 AGENTS.md + ledger (process only) → Phase 2 all gates to `baseline` with committed baselines and ratchet → Phase 3 retro-spec hotspots only (routes to `craftsman-spec` `recover`) → Phase 4 steady state (new work strict via sprout, strangle on touch). Phase state lives in `.craftsman/adoption.toml`, written by the CLI, so the gear resumes across sessions.
- `upgrade`: refresh the vendored conventions file and CLI version pin; report drift; never touch user content.

**CLI touchpoints:** `craftsman init`, `craftsman adopt --phase N`, `craftsman doctor`, `craftsman gate baseline <gate>`.

**Description (draft):** "Craftsman bootstrap — brings a repo under the methodology. Gears: new (greenfield: minimal AGENTS.md interview, craftsman.toml, verify harness, one proven red→green scenario), adopt (brownfield: observe → ledger → baseline gates → recover truth → steady state), upgrade (refresh conventions + CLI pin). Use for 'set up craftsman', 'init craftsman', 'adopt craftsman in this repo'. All gears write files — always confirms scope before writing. For drafting scenarios afterwards use craftsman-spec. Requires the craftsman CLI on PATH; if missing, point to the installer and stop."

### 2. craftsman-spec — the executable specification

**Gears:**

- `draft`\* : the librarian move — read the declared official docs (`craftsman docs`), run a lightweight Example-Mapping interview (rules, examples, open questions), write Gherkin scenarios following the authoring guidelines reference; scenarios follow the code-gen-friendly subset (no NL step ambiguity — each scenario maps to a generated test); human approves before SPEC.md changes land.
- `delta`: OpenSpec-style change specs — `ADDED / MODIFIED / REMOVED` scenarios against current truth, merged into SPEC.md when the change completes. The brownfield and long-lived-project mode.
- `recover`: brownfield truth recovery — generate characterization/approval tests at seams, human approves snapshots (pin vs file-as-bug), then draft scenarios labeled `verified`/`inferred`/`gap`; only `verified` scenarios enter SPEC.md.

**Rules:** SPEC.md is static during implementation; only the human changes acceptance criteria. The skill never marks a scenario passing — `craftsman verify` does.

**References:** `gherkin-authoring.md` (seeded from Knight's guidelines + code-gen constraints), `example-mapping.md`, `recover.md`.

**CLI touchpoints:** `craftsman docs search/get`, `craftsman spec lint` (scenario inventory, code-gen compatibility check), `craftsman verify --scenario`.

**Description (draft):** "Craftsman spec — the librarian turns official documentation into Gherkin SPEC.md scenarios the human approves; the spec is the test suite. Gears: draft (new scenarios from docs + example mapping — the default), delta (ADDED/MODIFIED/REMOVED change specs against current truth), recover (pin existing behavior via characterization tests; only verified scenarios enter the spec). Use for 'draft the spec', 'spec this feature', 'write scenarios', 'update the spec'. For batching use craftsman-plan; for building use craftsman-implement. Applies only inside a Craftsman project (craftsman.toml present); otherwise offer craftsman-init and stop."

### 3. craftsman-plan — batching and replanning

**Gears:**

- `batch`\* : group red scenarios into batches of 2–4 related scenarios; write PLAN.md with per-batch tasks and the mechanical success line (`craftsman verify --batch N` exits 0). Where the harness has a native plan mode, produce the thinking *through* it and persist the result as PLAN.md — the durable, harness-neutral artifact.
- `revise`: observation-grounded replanning after a batch — what did execution teach that invalidates or simplifies remaining batches.
- `gap`: re-read SPEC.md; verify every remaining red scenario is covered by a remaining batch; surface uncovered scenarios as new batches, never silently.

**Rules:** the plan moves, the spec doesn't. Batches stay small enough to execute without re-reading (the monolith-plan failure). PLAN.md never contains architecture prose — tasks and targets only.

**CLI touchpoints:** `craftsman spec status` (red/green inventory), `craftsman verify --batch`.

**Description (draft):** "Craftsman planning — batches SPEC.md scenarios into a lean PLAN.md the work follows. Gears: batch (group red scenarios into 2–4-scenario batches with mechanical success criteria — the default), revise (replan from what the last batch taught), gap (check remaining batches still cover all remaining red scenarios). Use for 'plan the batches', 'update the plan', 'what's next', 'replan'. Use the harness's native plan mode for the thinking; PLAN.md is the durable artifact. For execution use craftsman-implement. Applies only inside a Craftsman project; otherwise offer craftsman-init and stop."

### 4. craftsman-implement — execution

The largest skill; the librarian as builder.

**Gears:**

- `batch`\* : work the current batch task by task — docs grounding first (`craftsman docs`, no source no code), confirm target scenarios red, implement the minimum to green, refactor while green. No procedural TDD narration: the loop is enforced by *mechanical context* — `craftsman verify --impact <diff>` reports which scenarios and tests the change can break (the TDAD mechanism), and the agent runs it before and after each task. Recovery budgets per the conventions; three failed attempts → stop, report, draft ADR. Dependency additions run the five-point vetting protocol and carry the `Dependency:` trailer.
- `boundary`: the batch-boundary protocol, in order — full `craftsman verify` (regressions) → `craftsman check-all` (gates; bounded fix loops per gate) → gap pass (route to `craftsman-plan` `gap`) → extract durable learnings (`craftsman extract` writes `.craftsman/session/`; decisions and failed approaches only, nothing re-derivable) → ledger commit with trailers → stop and report; never auto-start the next batch.
- `finish`: when all scenarios are green — full QA pass, ADR consolidation proposal (human-gated), stale-ADR detection (cross-reference active ADRs against git history for the files they describe; flag, human decides), AGENTS.md accuracy check against reality, final commit.
- `quick`: **the light path.** For small scoped changes with no new externally visible behavior: skip SPEC/PLAN ceremony entirely; make the change; run `craftsman check-all --changed`; commit with `Verified-by:`. If the change turns out to grow behavior, stop and route to `craftsman-spec`.

**References:** `boundary.md`, `finish.md`, `quick.md`, `dependencies.md`, `testing.md` (emergent unit/integration tests — what to test and what not; property-based patterns for invariant-heavy code, roundtrip/oracle properties), and the per-stack idiom files — `stack-swift.md`, `stack-python.md`, `stack-typescript.md`, `stack-rust.md`, `stack-bash.md` — loaded on demand per `craftsman.toml`. Stack files carry only what tooling can't enforce (API-naming idiom, boundary patterns, pydantic-at-edges, thiserror/anyhow split, bash graduation rule); on Apple they defer to Xcode 27's exported skills for SwiftUI/testing idiom.

**CLI touchpoints:** `craftsman verify [--batch|--scenario|--impact]`, `craftsman check-all [--changed]`, `craftsman docs`, `craftsman extract`, `craftsman commit` (writes trailers).

**Description (draft):** "Craftsman execution — turns red scenarios green at production grade. Gears: batch (work the current PLAN.md batch: docs first, confirm red, minimal green, refactor — the default), boundary (batch end: all gates, gap check, extract learnings, ledger commit, stop), finish (all green: QA, ADR consolidation, final commit), quick (light path for small scoped changes: no spec ceremony, gates and Verified-by still mandatory). Use for 'implement', 'next batch', 'run the boundary', 'quick change'. Bugs go to craftsman-fix; new behavior to craftsman-spec first. Applies only inside a Craftsman project; otherwise offer craftsman-init and stop."

### 5. craftsman-fix — root-cause bug fixing

**Gears:**

- `diagnose`\* — mandatory first: reproduce mechanically, isolate (bisect, trace), single hypothesis, report before fixing. Pre-action gate: query `decisions/index.md` and `git log --grep="Rejected:"` for prior failed approaches before proposing one.
- `fix`: write the failing root-cause test (not symptom), apply the minimal fix, full verify, one commit — nothing else in it.
- `improve`: separate commit — health check on touched files; refactor in small verified steps only if health degraded or was already low; `Health-before/after` recorded.

**Rules:** no code before diagnosis; fix and refactor never share a commit; every fix leaves a root-cause regression test.

**CLI touchpoints:** `craftsman verify --scenario`, `craftsman health --files`, `craftsman commit`.

**Description (draft):** "Craftsman bug fixing — diagnose before touching code. Gears: diagnose (reproduce, isolate, hypothesize, report; checks the ledgers for previously rejected approaches — the default and mandatory first), fix (failing root-cause test + minimal fix, one clean commit), improve (separate health-restoring refactor commit, measured). Use for 'fix this bug', 'why is this failing', 'this scenario went red', 'regression'. Never mixes fix and refactor in one commit. Small non-bug changes go to craftsman-implement quick; new behavior to craftsman-spec. Applies only inside a Craftsman project; otherwise offer craftsman-init and stop."

### 6. craftsman-review — advisory judgment

**Gears:**

- `quality`\* : structural review — architecture, patterns, naming, complexity — against the AGENTS.md bar, with the mechanical evidence (`craftsman check-all` output, health scores) as input, not as something to re-litigate. Output: findings and suggestions, explicitly advisory. Complements, never replaces, harness/AI review products the team runs.
- `design`: front-end and API taste; defers to Impeccable when installed (probe, don't assume); on Apple, defers to Apple's exported skills for platform idiom.

**Rules:** this skill never says pass/fail on "does it work" — that is `craftsman verify`'s verdict alone. Invoked when the human asks, not on a schedule.

**Description (draft):** "Craftsman review — advisory judgment, never a gate. Gears: quality (architecture, patterns, naming, complexity against the AGENTS.md bar, using gate output as evidence — the default), design (front-end and API taste; defers to Impeccable and Apple's skills when installed). Use for 'review this', 'critique the architecture', 'is this code good'. Whether code works is craftsman verify's exit code, never this skill's opinion. Spec questions go to craftsman-spec; fixes to craftsman-fix. Applies only inside a Craftsman project; otherwise offer craftsman-init and stop."

## What the skills deliberately do NOT contain

| Concern | Lives in | Why |
|---|---|---|
| Pass/fail verdicts, scenario status, gate results | `craftsman` CLI exit codes + JSON | The machine actor; LLM-as-judge disqualified |
| PLAN/ledger/baseline/extract writes | CLI (single-writer) | Auditability; skills judge, code records |
| Style rules (formatting, imports, naming mechanics) | Linters via `craftsman lint` | Never restate what a tool enforces |
| Architecture rules (dependency direction, size caps) | `craftsman arch` fitness functions | Prose decays, gates don't (ETH result) |
| Session memory, compaction UX, checkpoints, plan-mode UX | Harness natives | Absorbed in 2025–26; integrate, don't compete |
| Project vision, taste, hard constraints | AGENTS.md (human-written, minimal) | The Instructions leg of the triad |
| SwiftUI/Swift-testing platform idiom | Apple's exported Xcode 27 skills | Compose, don't duplicate — drift-free |
| Design-domain depth | Impeccable (installed separately) | Adopt the domain incumbent |

## Traceability to design mandates

- **Light path** → `quick` gear + conventions scale-routing rule (§ Shared architecture, item 5).
- **Minimal AGENTS.md** → enforced by `init` interview budget; accuracy check at `finish`.
- **No TDD sermons** → `implement` `batch` gear is built around `craftsman verify --impact` mechanical context, not ritual language.
- **Integrate harness natives** → `plan` produces through native plan modes; extraction targets the repo only; no memory/checkpoint machinery anywhere.
- **AI review advisory** → `craftsman-review` rules; gate verdicts CLI-only.
- **Description budget** → six descriptions ≈ 3.2k chars total; each ≤ 600 chars, triggers front-loaded.

## Open items (next design steps)

1. **CLI command surface + `craftsman.toml` spec** — the skills above name their touchpoints (`verify --impact`, `spec lint`, `extract`, `commit`, `doctor`, `adopt --phase`); the CLI design doc must define them. `verify --impact` is new relative to the CLI research (TDAD mechanism) and needs a design.
2. **Conventions file content** — draft `craftsman-conventions.md` verbatim (target ≤ 160 lines, Fusion's is 161).
3. **`--batch` semantics** — README design decision #1; the `plan`/`implement` contract above works with either answer; decide at CLI design time.
4. **Skill authoring pass** — write the six SKILL.md files + references against the skill-shaper/agentskills.io checklist; then the e2e dogfood loop (Superpowers-style tests of the methodology itself).
