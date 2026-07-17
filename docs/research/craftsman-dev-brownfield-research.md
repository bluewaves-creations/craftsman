# Craftsman Dev on Existing Codebases: Brownfield Adoption

> Retrofitting spec-driven, gate-driven agentic development onto legacy code — retro-speccing, delta specs, quality ratchets, and the strangler fig, evaluated against the 2025–2026 landscape.

---

## The Problem

Craftsman Dev assumes greenfield: write AGENTS.md, draft SPEC.md in Gherkin from official docs, batch PLAN.md, implement until green, gate every batch mechanically. An existing codebase breaks every one of those assumptions:

- **There is no SPEC.md** — the spec is implicit in the code's behavior, including its bugs, which users may depend on (Hyrum's Law).
- **The gates would all be red on day one** — turning on `craftsman lint/health/security` against a 5.15-average-health codebase produces thousands of failures and an agent that either drowns fixing them or learns to ignore red.
- **AGENTS.md doesn't exist**, and generating one by inference risks documenting an architecture the code doesn't actually have.
- **The three-actor model inverts**: on greenfield the human knows the vision and the code doesn't exist; on brownfield the code knows things no human remembers.

The question: what is the ordered, evidence-based protocol for getting an existing repo from "no methodology" to "full Craftsman Dev" without a big-bang rewrite of either the code or the process?

## 1. Retro-Speccing: Extracting Specs From Behavior

### Characterization tests are the ground truth layer

Michael Feathers' characterization tests (2004) remain the foundational move, and the legacy-code literature is explicit that characterization, Golden Master, approval, and snapshot testing are **the same technique under different names** (understandlegacycode.com). The purpose is inverted from TDD: *document what the system actually does, not what you wish it did*. The test is generated from observed output, approved by a human, and thereafter fails on any behavioral change.

This maps perfectly onto Craftsman's Machine actor: an approval test is a mechanical, exit-code verdict about behavior preservation — no LLM opinion involved. Per-stack tooling is mature:

| Stack | Tool | Baseline mechanism |
|---|---|---|
| Swift (Apple + OSS) | swift-snapshot-testing (Point-Free) | recorded snapshots per assertion |
| Python | ApprovalTests.Python, syrupy | `.approved.` files / snapshot dirs |
| TypeScript | Jest/Vitest snapshots, ApprovalTests | `__snapshots__/`, inline snapshots |
| Rust | insta (`cargo insta review`) | `.snap` files + interactive review |
| .NET (reference) | Verify | `.verified.` files + diff tooling |
| Any CLI/bash | Golden Master scripts, `diff` against recorded output | recorded stdout/stderr/exit codes |

The critical craft rule from the approval-testing literature: the human **approves** the snapshot — the recorded behavior may be a bug, and approval is the moment to decide whether to pin it (compatibility) or file it (defect). An agent can *generate* characterization tests at scale; only the human can *approve* them as intended truth vs. pinned accident.

```rust
// insta (Rust): pin current behavior, review interactively with `cargo insta review`
#[test]
fn characterize_invoice_rounding() {
    // NOTE: rounds half-down — possibly a bug, pinned 2026-07 pending ADR-014
    insta::assert_snapshot!(format_invoice_total(dec!(10.005)));
}
```

```python
# ApprovalTests.Python: golden master over a whole legacy report
def test_characterize_monthly_report():
    verify(generate_report(fixture_ledger()))  # diff against .approved. file
```

### Agent-assisted spec recovery (2026 state of the art)

- **Reversa** (arXiv 2605.18684, May 2026) is the closest existing system to "retro-spec for Craftsman": a multi-agent reverse-documentation pipeline (Scout → Archaeologist → Detective → Architect → Writer → Reviewer) that converts legacy code into operational specs with **confidence-classified claims** (confirmed / inferred / explicit gaps), traceability matrices, and **executable Gherkin parity scenarios** (53 scenarios for a COBOL→Go ATM case study; 517 claims at 97.1% self-reported confidence). *Caveat, clearly marked: single educational case study, no external accuracy audit, no controlled baseline — treat as a design pattern, not validated tooling.*
- **CodeSpecBench** (arXiv 2604.12268) benchmarks LLMs on generating executable behavioral specs (pre/postconditions) — evidence that spec generation quality is measurable, not vibes.
- **DiffSpec** (arXiv 2410.04249) uses specs + code + historical bugs to generate differential tests — relevant to verifying a strangler-fig replacement against the legacy original.

The transferable Reversa insight is the **confidence taxonomy**: a retro-spec'd scenario must be labeled `verified` (backed by a passing characterization test), `inferred` (agent's reading of the code, unexecuted), or `gap` (unknown, needs human). Only `verified` scenarios count as spec truth.

## 2. OpenSpec's Brownfield-First Model — and Spec Kit's

**OpenSpec** (Fission-AI, MIT) is the only major SDD framework designed brownfield-first, and its two structural decisions are directly stealable:

1. **`openspec/specs/` vs `openspec/changes/`** — specs/ is *accumulated current truth* organized by capability (auth, payments…); changes/ holds *proposed deltas* written as `ADDED` / `MODIFIED` / `REMOVED` requirements against that truth. The workflow is propose → (explore) → apply → archive, and **archiving merges the delta into specs/**. Truth accretes change by change.
2. **"Resist the urge to back-fill everything"** — OpenSpec's existing-projects guide is emphatic: you do not document the whole codebase to start; you write specs only for what you're about to change. Backfilled documentation for code not under active change goes stale because nothing forces it to track reality.

**GitHub Spec Kit** is greenfield-leaning but its community converged on the same answer (discussion #331, brownfield extensions, EPAM's write-up): incremental per-change specs plus an agent-produced *research doc* about the existing code that specs reference; and the constitution becomes **archaeological** — "here is how this codebase already works, and the agent must respect it," reading like a description of reality rather than aspiration.

The gap in both — and Craftsman's opportunity: **neither verifies the recovered spec mechanically.** OpenSpec's specs/ is truth by declaration; nothing proves the ADDED/MODIFIED delta or the underlying "current truth" matches the running system. Craftsman's Machine actor closes this: a spec section for existing behavior is only admitted to SPEC.md once a characterization test executes it green.

## 3. Incremental Gates: Baseline + Hold-the-Line + Ratchet

The universal pattern across every 2025–2026 tool: **record existing violations as a baseline, fail only on new ones, and ratchet the baseline monotonically down.** Concretely, per Craftsman gate and stack:

| Gate | Tool | Baseline / ratchet mechanism |
|---|---|---|
| lint (TS) | ESLint ≥ 9.24 bulk suppressions | `eslint --suppress-all`, `--prune-suppressions` to ratchet |
| lint (Swift) | SwiftLint ≥ 0.55 | `--write-baseline` / `--baseline`; new code fully enforced |
| lint (Python) | Ruff | **no native baseline** (issue #1149 open as of writing) — use `--add-noqa` for inline suppression, per-file-ignores, staged rule enablement |
| lint (Rust) | Clippy | `#[allow]` at module scope + `-D warnings` on new crates; no baseline file |
| security | Semgrep | diff-aware scans: `SEMGREP_BASELINE_REF=<ref> semgrep ci` reports only findings introduced after the baseline commit |
| health | CodeScene | delta analysis + PR gates: goals per hotspot, "supervise so it doesn't grow worse," block merges that drop health; ACE agent "limits itself to new degradations" |
| anything measurable | Betterer | Jest-style snapshot of any metric into `.betterer.results`; worse = error, better = auto-tightened snapshot |
| verify | characterization suite | the approval-test corpus *is* the baseline for behavior |

The concrete baseline commands, per stack:

```bash
# Swift — SwiftLint ≥ 0.55
swiftlint lint --write-baseline .swiftlint-baseline.json     # record debt once
swiftlint lint --baseline .swiftlint-baseline.json --strict  # CI: new violations only

# TypeScript — ESLint ≥ 9.24
npx eslint --fix --suppress-all .        # record (auto-fixing what it can)
npx eslint .                             # CI: suppressed violations stay silent
npx eslint --prune-suppressions .        # ratchet: drop entries that no longer fire

# Python — Ruff (no native baseline; issue #1149 open)
ruff check --add-noqa .                  # inline-suppress existing violations
ruff check .                             # CI: new violations only

# Security — Semgrep diff-aware
SEMGREP_BASELINE_REF=$(git merge-base main HEAD) semgrep ci

# Anything else — Betterer snapshots any countable metric
npx betterer            # worse = exit 1; better = snapshot auto-tightens
```

Two mechanisms deserve special note:

- **Betterer's auto-ratchet** is the strictest form: when a metric improves, the snapshot tightens *automatically and permanently* — regression to the old (worse) level now fails. Notion's custom ESLint ratcheting system runs the same policy at scale in production.
- **ESLint's `--prune-suppressions`** is the maintenance half everyone forgets: a baseline that only grows stale is a suppression graveyard. The ratchet needs a scheduled pruning step.

For Craftsman this means every gate gets three modes: `off` → `baseline` (fail only on new violations vs. recorded baseline; auto-ratchet on improvement) → `strict` (greenfield behavior). Baseline files are committed, and the ledger records each ratchet event (`Verified-by:` a shrinking count).

## 4. Legacy Strategy Classics, Agent Edition

- **Seams + characterization (Feathers).** The 2026 practitioner consensus (Feathers' own Tech Lead Journal episode #195; Sourcegraph's legacy modernization guide) is that **AI amplifies process, good or bad**: a team without tests gets *worse* output from agents, not better — the same mechanism as CodeScene's 60%-more-defects finding, from the process side. The sequence stands: find a seam → break the dependency minimally → pin behavior with characterization tests → only then let the agent change anything. Agents change the economics, not the sequence: writing 500 characterization tests was weeks of human tedium; it's now hours of agent work plus human approval passes.
- **Sprout method.** When the area can't be brought under test yet, *sprout*: new behavior goes in a new, fully-Craftsman function/module that the legacy code calls. Sprouted code is greenfield — full SPEC.md scenarios, strict gates — inside a brownfield host. This is the micro-scale strangler fig.
- **Strangler fig at scale.** AWS Transform's mainframe "reimagine" pattern is a productized agentic strangler fig with exactly Craftsman's shape: *reverse engineering* (extract business logic from COBOL/JCL) → *forward engineering* (generate microservice **specifications**, then code) → *deploy and test* behind a routing facade. Spec extraction precedes code generation. Industry reporting claims 40–50% timeline reductions from AI-assisted modernization vs 2023 (*vendor/industry figures — not independently verified*).
- **Google's LLM migrations** (FSE 2025, arXiv 2504.09691): 39 migrations, 595 changes, 93,574 edits — 74% of changes LLM-generated, ~50% total-time reduction, with the pipeline anchored on **deterministic change-location discovery plus existing test suites as the verification oracle**. Same lesson: the machine-verifiable harness, not the LLM, is what made scale safe.

## 5. AGENTS.md for Existing Repos

Generating AGENTS.md by pointing an agent at the repo (`/init` and equivalents) is tempting and measurably hazardous. An ETH Zurich study (*reported via 2026 secondary coverage; primary paper not independently verified*) found LLM-generated context files **reduced task success ~2–3% and increased inference cost 20–23%**, mostly by duplicating what the agent can read from source anyway; architectural overviews specifically increased cost and encouraged broader file traversal without improving success. Addy Osmani's "Stop Using /init" and the aihero.dev "Never Run Claude /init" pieces converge on the same editorial rule:

- **High signal:** commands that actually build/test/run, non-inferable constraints, counterintuitive decisions ("we don't use X because Y"), tribal knowledge, custom tooling.
- **Negative signal:** inferred architecture narratives, directory listings, restated code — stale structural references *actively mislead*.

For brownfield Craftsman: AGENTS.md must be **observed, not inferred** — every line either a command that was executed successfully during Phase 0 or a fact a human attested. Inferred architecture goes into a separate, explicitly-labeled research doc (the Spec Kit pattern), never into AGENTS.md. The Reversa confidence taxonomy applies here too.

## The Adoption Sequence

Proposed phased protocol, each phase grounded in the evidence above. Phases are cumulative; nothing turns strict before its baseline exists.

```
Phase 0: OBSERVE          Phase 1: LEDGER         Phase 2: HOLD THE LINE
read-only mapping    →    AGENTS.md (observed) →  all gates: baseline mode
research doc              trailers + ADR-000      committed baselines, ratchet
(no code changes)         (no code changes)       (codebase can't get worse)
                                                          │
                              ┌───────────────────────────┘
                              ▼
Phase 3: RECOVER TRUTH            Phase 4: STEADY STATE
characterization tests       →    new work: full Craftsman, strict (sprout)
verified scenarios → SPEC.md      old code: strangle on touch, ratchet to
(hotspots + critical only)        zero, flip module to strict — permanently
```

**Phase 0 — Observe (days).** Read-only. Agent maps the repo (Spec Kit "research doc" pattern; Reversa Scout/Archaeologist roles): entry points, build/test/run commands *executed and verified*, hotspots (CodeScene or `git log` churn × complexity), test coverage reality. Output: a confidence-labeled research doc + the verified command list. No code changes.

**Phase 1 — AGENTS.md + ledgers (days).** Write AGENTS.md from Phase 0 observations only (observed-not-inferred rule). Turn on Craftsman commit trailers (`Learned:/Rejected:/Verified-by:`) and `decisions/` ADRs — process-only, zero code risk, and the ledger starts capturing archaeology immediately (ADR-000: "state of the system at adoption").

**Phase 2 — Baseline gates, hold the line (week).** Every gate goes to `baseline` mode: SwiftLint `--write-baseline`, ESLint `--suppress-all`, Semgrep `SEMGREP_BASELINE_REF`, CodeScene delta analysis, Betterer for anything without native support (incl. Ruff until #1149 lands). Baselines committed; CI fails on *new* violations only; auto-ratchet + scheduled pruning. From this commit forward, the codebase cannot get worse — the CodeScene/Feathers precondition for agent work at all.

**Phase 3 — Retro-spec critical paths (weeks, ongoing).** For the top hotspots and revenue-critical flows only (never whole-codebase — OpenSpec's backfill warning): agent generates characterization/approval tests at seams → human approves snapshots (pin vs. file as bug) → agent drafts Gherkin scenarios from approved behavior, labeled `verified`/`inferred`/`gap` → only `verified` scenarios enter SPEC.md under a "Current behavior (recovered)" section. SPEC.md adopts OpenSpec's split: recovered truth vs. proposed deltas.

```gherkin
# SPEC.md — Current behavior (recovered)
Scenario: Expired session redirects to login   # confidence: verified
  # Verified-by: tests/characterization/test_session.py::test_expired_redirect
  Given a session older than 30 minutes
  When the user requests any authenticated page
  Then they are redirected to /login with the original URL preserved

Scenario: Concurrent checkout double-charges    # confidence: gap — needs human
  # Observed in support tickets; no seam found yet. Tracked as GAP-007.
```

```
retro-spec(auth): pin session expiry behavior

12 characterization tests added at SessionMiddleware seam.
2 snapshots filed as bugs (BUG-114, BUG-115), not pinned.
3 scenarios promoted to SPEC.md as verified.

Learned: expiry uses server clock, not token iat — pinned deliberately
Verified-by: craftsman verify --scope auth
```

**Phase 4 — Full Craftsman on new work; strangler on old (steady state).** All new features run the complete greenfield loop (spec → plan → batches → strict gates) via sprout method — new modules are strict-mode islands. Legacy areas are strangled opportunistically: when a change touches an unspec'd area, retro-spec that area first (delta-scoped, OpenSpec-style), then change it. Ratchets tighten until a module's baseline hits zero, at which point it flips to `strict` permanently. The methodology spreads through the codebase the way the fig spreads through the tree.

## What Craftsman Dev Should Adopt

1. **The specs-as-truth / changes-as-delta split** (OpenSpec) — SPEC.md gains a recovered-truth section; all brownfield change proposals are written as ADDED/MODIFIED/REMOVED deltas against it, merged on completion.
2. **Characterization tests as the retro-spec verifier** — a recovered Gherkin scenario is only `verified` when a mechanical approval test executes it. This is Craftsman's differentiator over OpenSpec/Spec Kit: recovered specs are *proven*, not declared.
3. **Three-mode gates (`off`/`baseline`/`strict`) with committed baselines, auto-ratchet, and pruning** — `craftsman gate baseline <gate>` wraps each tool's native mechanism (or Betterer where none exists), and the ledger records ratchet events.
4. **Confidence labels on everything recovered** (Reversa): `verified` / `inferred` / `gap` — on scenarios, on research docs, on AGENTS.md candidates. Gaps are work items, not silence.
5. **Observed-not-inferred AGENTS.md** — generated from executed commands and human attestations; inferred architecture quarantined in a labeled research doc.
6. **Sprout + strangler as the expansion strategy** — strict-mode islands in new code; per-module graduation from baseline to strict at zero violations.

## What NOT to Adopt

- **Whole-codebase spec backfill.** Every source agrees (OpenSpec explicitly, Spec Kit community empirically): comprehensive retro-documentation goes stale and burns the budget. Retro-spec only what is hot, critical, or about to change.
- **Unverified agent-generated specs as truth.** Reversa's 97.1% confidence was *self-reported with no external audit*. An LLM's reading of code is an `inferred` claim until a test executes it — admitting it to SPEC.md unverified is LLM opinion as ground truth, the exact thing Craftsman rejects.
- **Auto-generated architecture narratives in AGENTS.md.** Measured to reduce success and raise cost; stale structure actively misleads.
- **Permanent baselines.** A baseline without ratchet + pruning is institutionalized debt with a config file. Baseline mode is a transition state with a monotonic exit, never a destination.
- **Big-bang strict mode.** Turning all gates strict on day one either halts work or trains everyone (human and agent) that red is normal. A gate that is always red verifies nothing.

## Conclusion

The brownfield problem decomposes cleanly onto Craftsman's three actors. The **Machine** extends naturally: characterization tests make "what the system does today" a mechanical, exit-code fact, and baseline+ratchet mechanisms make "no new debt" mechanically enforceable from day one — before a single spec exists. The **Agent** does what was previously uneconomical: generating characterization suites, mining candidate specs with confidence labels, and executing strangler migrations at Google-demonstrated scale. The **Human** keeps the two irreducible brownfield judgments: approving snapshots (is this pinned behavior intended or a bug?) and attesting AGENTS.md facts.

The ordering is the whole game: *ledger before gates, gates before specs, specs before change.* Hold the line first, recover truth second, build new work greenfield-strict third, and strangle the rest at the pace the ratchet allows. Craftsman's contribution over the existing SDD frameworks is the verification spine — OpenSpec knows how to *structure* recovered truth, but only a characterization test can *prove* it.

### Key sources

- OpenSpec brownfield guide: github.com/Fission-AI/OpenSpec (docs/existing-projects.md, docs/concepts.md)
- Spec Kit brownfield: github/spec-kit discussion #331, issue #1436; intent-driven.dev; EPAM insights
- Reversa (arXiv 2605.18684); CodeSpecBench (arXiv 2604.12268); DiffSpec (arXiv 2410.04249)
- Migrating Code at Scale with LLMs at Google (FSE 2025, arXiv 2504.09691)
- ESLint bulk suppressions (eslint.org/blog 2025/04); SwiftLint baseline (0.55+); Semgrep diff-aware (SEMGREP_BASELINE_REF); Ruff issue #1149 (open); Betterer; Notion ESLint ratcheting
- CodeScene delta analysis / PR gates; understandlegacycode.com on characterization = approval = golden master; Feathers, Tech Lead Journal #195; AWS Transform mainframe reimagine pattern; Addy Osmani "Stop Using /init"; ETH Zurich context-file study (via secondary coverage — unverified)
