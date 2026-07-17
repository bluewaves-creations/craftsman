# Craftsman Dev — Research Corpus

Twenty documents. Foundations first, then process deep dives, then the mechanical gates, then the build-phase research (commissioned 2026-07-17) that closes the gaps between methodology and implementation.

## Foundations

- **[craftsman-dev-methodology.md](craftsman-dev-methodology.md)** — the constitution: three-actor model (human/agent/machine), five artifacts (AGENTS.md → SPEC.md → PLAN.md → git ledger → decisions/), batch workflow, non-negotiable constraints.
- **[the-triad-pattern.md](the-triad-pattern.md)** — the architecture: Instructions (strategy) / Skills (knowledge, on demand) / CLI (deterministic action, zero prompt cost); layer-boundary tests and anti-patterns.
- **[superpowers-skills-token-analysis.md](superpowers-skills-token-analysis.md)** — why a lean skill catalog with progressive disclosure is the right economics; where tokens actually go.

## Process deep dives

- **[craftsman-dev-spec-research.md](craftsman-dev-spec-research.md)** — Gherkin vs the 2026 SDD wave; keep Gherkin, add property-based testing and Example Mapping.
- **[craftsman-dev-adaptive-planning-research.md](craftsman-dev-adaptive-planning-research.md)** — batch boundary protocol, gap-finding, failure-classified recovery budgets, why static plans lose.
- **[craftsman-dev-testing-pyramid-research.md](craftsman-dev-testing-pyramid-research.md)** — acceptance (SPEC.md) on top; unit/integration/property tests as implementation artifacts.
- **[craftsman-dev-bugfix-research.md](craftsman-dev-bugfix-research.md)** — Diagnose → Fix → Improve; fix and refactor are separate commits; code health as a fix gate.
- **[craftsman-dev-implementation-quality-research.md](craftsman-dev-implementation-quality-research.md)** — HC/Convention/Style tiers in AGENTS.md, show-don't-tell, doc grounding, CodeHealth gates.
- **[craftsman-dev-ledgers-research.md](craftsman-dev-ledgers-research.md)** — git trailers (Learned:/Rejected:/Verified-by:) + consolidated ADRs + stale-memory detection + pre-action gate.
- **[craftsman-dev-compaction-research.md](craftsman-dev-compaction-research.md)** — extraction to `.craftsman/session/` before compression; progressive-disclosure session state.
- **[craftsman-dev-architecture-fanout-research.md](craftsman-dev-architecture-fanout-research.md)** — fitness functions, repository maps, AGENTS.md boundaries; selective fan-out in worktrees.

## Mechanical gates

- **[craftsman-dev-security-research.md](craftsman-dev-security-research.md)** — secret scan + SAST + SCA as batch-boundary gates.
- **[craftsman-dev-dependencies-research.md](craftsman-dev-dependencies-research.md)** — five-point vetting protocol, lockfile integrity, dependency commit trailers.
- **[craftsman-dev-performance-research.md](craftsman-dev-performance-research.md)** — Lighthouse CI, k6, size-limit as performance fitness functions.
- **[craftsman-dev-frontend-design-research.md](craftsman-dev-frontend-design-research.md)** — distributional convergence, design tokens, Playwright/axe-core/token-lint gates.

## Build-phase research (2026-07-17)

- **[craftsman-dev-agent-agnostic-skills-research.md](craftsman-dev-agent-agnostic-skills-research.md)** — the house pattern from Fusion/Shaping Rooms (four-point skill contract, description formula, thin-bootstrap installer, tree-hash vendoring), the agentskills.io standard, the harness matrix (`~/.agents/skills` canonical), catalog token budgets, supply-chain posture.
- **[craftsman-dev-verification-cli-research.md](craftsman-dev-verification-cli-research.md)** — Rust + clap CLI, per-stack runner adapters (pytest-bdd / cucumber-js / playwright-bdd / cucumber-rs / Swift Testing code-gen / bats code-gen), owned result schema with Cucumber Messages status vocabulary, trunk-check architecture, agent-first output contract, cargo-dist distribution.
- **[craftsman-dev-brownfield-research.md](craftsman-dev-brownfield-research.md)** — characterization tests as retro-spec verifier, OpenSpec specs/changes split, three-mode gates (off/baseline/strict) with ratchet, observed-not-inferred AGENTS.md, the five-phase adoption sequence.
- **[craftsman-dev-xcode27-composability-research.md](craftsman-dev-xcode27-composability-research.md)** — Xcode 27's seven exportable Agent Skills (confirmed), the GUI-bound vs headless split, the CLI verification surface (xcodebuild/xcresulttool/swift test/simctl/AXe/swift-snapshot-testing), Gherkin → Swift Testing code-gen.
- **[craftsman-dev-documentation-pipeline-research.md](craftsman-dev-documentation-pipeline-research.md)** — `craftsman docs` CLI-first grounding (per-page .md, llms.txt, docs.rs JSON, DocC export, objects.inv, .d.ts), first-party MCPs as accelerators (Cloudflare, Xcode, MS Learn), the AGENTS.md Documentation Sources table, mechanical grounding verification (type checkers, deprecation lints, API differs).

## Validation research (2026-07-17)

- **[craftsman-dev-competitive-landscape-research.md](craftsman-dev-competitive-landscape-research.md)** — the honest audit: five verified-empty niches to build (Gherkin executable spec, cross-stack verify adapters, static+runtime gate orchestration with ratchet, git-trailer ledgers, thin docs pipeline); wrap qlty/trunk, adopt Impeccable/Trail of Bits, steal Superpowers' review split and token diet; drop session-side compaction/planning UX to harness natives; "Where We Would Lose Today" and the bear case.
- **[craftsman-dev-production-grade-research.md](craftsman-dev-production-grade-research.md)** — evidence-graded: mechanical impact-mapped verification (TDAD −70% regressions), LLM-as-judge disqualified as a gate (52–78%), minimal human-written AGENTS.md (verbose/generated files measurably harm), diff-scoped mutation testing as a new gate, ratcheted fitness functions against quantified agentic entropy, per-stack 2026 architecture consensus, and the runnable 8-task × 3-condition eval protocol.

## Standing decisions (2026-07-17)

- **Audience**: the author's team only. Manual installation is acceptable — marketplace/npm-wrapper/mass-distribution machinery from the agent-agnostic and verification-cli docs is optional, not required. Keep the thin installer + tested `craftsman setup` brain; drop the rest until needed.
- **Apple environment**: Xcode 27 confirmed as the team's actual iOS/macOS toolchain — the Xcode 27 composability doc describes the real target, not a hypothesis.
- **Bar to clear before building**: no wheel-reinvention, and best-in-class output quality — audited by the competitive-landscape and production-grade research docs.

## Open questions carried into the design phase

Design decisions (ours to make, no further research needed) — **all six settled 2026-07-17 in [../design/2026-07-17-cli-surface-design.md](../design/2026-07-17-cli-surface-design.md)**; kept here for the audit trail:

1. **`--batch` semantics** — Gherkin tags (`@batch-2`) vs a PLAN.md-side scenario list (verification-cli).
2. **Baseline format** — wrap each tool's native baseline vs one unified Betterer-style snapshot (brownfield).
3. **Skills bundling under a Rust CLI** — the house pattern bundles skills into a Python wheel via hatch; with cargo-dist the equivalent is embedding skills in the binary or shipping them as a release artifact, with `craftsman setup` still the tested brain (agent-agnostic × verification-cli). Simplified by the team-only audience decision.
4. **`craftsman docs` search** — pure ripgrep over the cache vs a small FTS index (documentation-pipeline).
5. **Lint/security gate internals** — wrap qlty vs trunk check (qlty currently the stronger candidate; trunk reads maintenance-mode) (competitive-landscape).
6. **Gate enforcement on Codex** — no hook system exists there; proposed answer is convention + `Verified-by:` trailers + CI backstop (competitive-landscape).

Design mandates from the validation research (non-optional, evidence-backed):

- **A scale-adaptive light path** — full ceremony on a one-line fix is the documented SDD failure mode; without a light default, "best-in-class efficiency" fails (competitive-landscape).
- **Minimal, human-written AGENTS.md with a hard length budget**; never auto-generated; every mechanizable rule becomes a gate (production-grade, ETH result).
- **No TDD sermons in skills** — mechanical impact context, not procedural ritual (production-grade, TDAD).
- **Diff-scoped mutation testing** joins the gate stack for Python/TS/Rust; coverage is a floor, never a target (production-grade).
- **Integrate, don't compete, with harness natives**: PLAN.md batches feed native plan modes; compaction reduces to extraction-of-durable-learnings-to-repo; session memory/checkpoints stay native (competitive-landscape).
- **AI review is a complement, never a gate** — pair mechanical verify with advisory LLM review for semantic bugs gates can't see (both docs).

Verification spikes (small experiments during early build):

5. `swift test --experimental-event-stream-output` behavior on Linux (verified on macOS only).
6. The Gherkin → Swift Testing code generator — highest-risk component, prototype first.
7. DocC `--enable-experimental-markdown-output` API stability before building the Swift doc cache on it.
8. ~~Exact export flag shape and Apple skill-name stability~~ **Resolved 2026-07-18 on Xcode 27 GA**: command is `xcrun mcpbridge run-agent skills export [--output-dir <path>] [--replace-existing]` (default `./xcode-skills`); seven skills confirmed, two names differ from beta coverage — `modernize-tests` (not `test-modernizer`) and `adopt-c-bounds-safety` (not `c-bounds-safety`). Exports are spec-conformant SKILL.md + references/; `device-interaction` self-declares as a subagent skill requiring an Xcode session (IDE-bound confirmed).

Watch items (external, recheck before 1.0):

9. Zed's skill scan paths (unverified); Windsurf paths post-acquisition; whether Claude Code gains native `.agents/skills` scanning.
10. Ruff native baseline support (issue #1149); pytest-bdd 8.2 release.
11. Swift Package Index MCP/llms.txt initiative (none found as of 2026-07).
12. Context7 keyless rate limits (undocumented); ETH Zurich context-file study primary source (cited via secondary coverage only).
13. Human approval workflow at scale when agents generate thousands of characterization tests (sampling / risk-tiered approval).
14. The two empty niches, rechecked before 1.0: a funded Gherkin-as-agent-spec entrant appearing; qlty adding runtime gates (competitive-landscape).
15. Swift mutation-testing tool maturity (muter not production-consensus); SwiftUI MV-vs-TCA consensus and property-based-testing efficacy need a follow-up verification pass (production-grade).
