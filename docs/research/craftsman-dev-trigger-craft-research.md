# Trigger Craft: What Superpowers 6.1.1 Knows About Making Skills Fire

> Superpowers is about to be removed from this machine so Craftsman can be dogfooded without interference. This document harvests its trigger maturity first — the three-layer trigger architecture, the description craft, the honest accounting of where it over-fires — and converts the lessons into revised Craftsman descriptions, a conventions-file rationalization table, and the dogfood-phase trigger-test matrix.

---

## The Question

Superpowers (Jesse Vincent, v6.1.1, 14 skills) is the most battle-tested answer to the problem Craftsman is about to face: models undertrigger skills, and an unfired skill is dead weight. Its answer is maximalist — force the check every time, preempt every excuse. Craftsman's answer is structural — an applicability gate, scale routing, human-owned artifacts. Before deleting Superpowers, what does its maturity teach us, and where does its strategy fail in exactly the ways Craftsman was designed to escape?

## Superpowers' Trigger Architecture, Dissected

Three layers, each attacking a different failure point:

| Layer | Mechanism | Why it works | Token cost |
|---|---|---|---|
| 1. SessionStart hook injection | `hooks/session-start` reads the full `using-superpowers/SKILL.md` and injects it wrapped in `<EXTREMELY_IMPORTANT>` on `startup\|clear\|compact` | The meta-skill cannot be un-discovered: it is in context before the first user message, and re-injected after compaction (the moment mandates usually evaporate) | ~430 words ≈ 600 tokens, every session, unconditionally |
| 2. The meta-skill's mandate | The 1%-chance rule ("even a 1% chance a skill might apply → you ABSOLUTELY MUST invoke"), timing anchor ("BEFORE any response — including clarifying questions"), a 12-row red-flags table pairing each rationalization with its rebuttal, priority rule (process skills before implementation skills), announce mandate | It fights the *next* failure after discovery: rationalized non-use. The red-flags table is preemptive — it names the exact thoughts the model will have ("this is just a simple question", "the skill is overkill") and brands each one a stop signal before the model has it | Included in layer 1's payload; the table is ~150 tokens of it |
| 3. Per-skill description craft | Symptom-based, timing-anchored, "Use when…" descriptions; imperative MUST framing on brainstorming | Catches the tasks the meta-skill's general rule can't route by itself; descriptions are the only layer other harnesses see | 14 descriptions ≈ 2.1k chars ≈ ~520 tokens catalog-resident |

A fourth mechanism operates *after* triggering — in-body compliance: `<HARD-GATE>` blocks (brainstorming), Iron Laws in fenced blocks ("NO FIXES WITHOUT ROOT CAUSE INVESTIGATION FIRST"), per-skill rationalization tables (TDD carries 11 rows; systematic-debugging 8; verification 8), red-flag lists ending "ALL of these mean: STOP", mandatory checklists ("create a task for each item"), and "Announce at start" lines in four skills. Superpowers treats triggering and compliance as one continuous war against rationalization; the same table pattern serves both fronts.

## Description-Craft Inventory

Techniques across the 14 descriptions:

| Technique | Example skills | Pattern |
|---|---|---|
| Symptom triggers | systematic-debugging | fire on the *situation*, not the noun ("any bug, test failure, or unexpected behavior") |
| Timing anchors | systematic-debugging, TDD, verification | "before proposing fixes", "before writing implementation code", "before committing or creating PRs" — attach the skill to a moment in the agent's own loop |
| Self-state triggers | verification-before-completion | fire on the model's *internal state*: "when about to claim work is complete, fixed, or passing" |
| Imperative MUST | brainstorming | "You MUST use this before any creative work" — the only description in second person; deliberate, and the family's over-trigger champion |
| "Especially if" escalation | receiving-code-review | "especially if feedback seems unclear or technically questionable" — sharpen the trigger where the model is most likely to skip |
| Universal quantifiers | using-superpowers, brainstorming | "ANY response", "any conversation", "any creative work" — trades precision for recall |
| Value statement tail | verification | "; evidence before assertions always" — a slogan the model can retrieve later |

The best five, verbatim:

1. `Use when encountering any bug, test failure, or unexpected behavior, before proposing fixes` — symptom + timing anchor in thirteen words.
2. `Use when about to claim work is complete, fixed, or passing, before committing or creating PRs - requires running verification commands and confirming output before making any success claims; evidence before assertions always` — the self-state trigger.
3. `Use when starting any conversation - establishes how to find and use skills, requiring skill invocation before ANY response including clarifying questions` — the timing anchor taken to its limit.
4. `You MUST use this before any creative work - creating features, building components, adding functionality, or modifying behavior.` — maximum-recall MUST framing.
5. `Use when receiving code review feedback, before implementing suggestions, especially if feedback seems unclear or technically questionable - requires technical rigor and verification, not performative agreement or blind implementation` — trigger sharpened exactly where compliance is weakest.

One internal contradiction worth noting: Superpowers' own writing-skills SDO doctrine says descriptions must state *only* when to use, never summarize workflow ("descriptions that summarize workflow create a shortcut agents will take"). Two of its flagship descriptions (verification, receiving-code-review) violate this. Craftsman's gear-enumerating descriptions also technically violate it — defensibly, because gear lists are *routing* information (which mode of this skill), not workflow the agent could execute instead of reading the body. The rule to keep: never put procedural steps in a description; gear names and their one-phrase identities are routing, not procedure.

## False-Positive Analysis (Honest)

**Where the strategy over-triggers.** The 1% rule is mathematically an over-trigger machine: at a 1% threshold, essentially every utterance clears the bar for some skill. The red-flags table then removes the escape hatches ("Questions are tasks. Check for skills." / "The skill is overkill → Simple things become complex. Use it."), and brainstorming's `<HARD-GATE>` explicitly extends design-approval ceremony to "EVERY project regardless of perceived simplicity — a todo list, a single-function utility, a config change." Superpowers' justifying argument is cost asymmetry: an unnecessary skill load costs a few thousand tokens; a missed skill costs the methodology. That argument is correct for its target user — someone bootstrapping discipline into an undisciplined loop.

**Where the asymmetry inverts: expert contexts with standing artifacts.** This session is the observable evidence. Superpowers is installed here, so under its own rules this document — "creative work — creating features" — required the brainstorming skill: clarifying questions one at a time, 2–3 approaches, a design doc in `docs/superpowers/specs/`, and user approval before writing. But the work is already governed by a 22-document research corpus, an approved design doc (`2026-07-17-skill-family-design.md`), and a task brief that specifies the deliverable's structure section by section. Brainstorming here is pure ceremony — re-approving decisions the human already made and recorded. That hard-gate friction on already-designed work is precisely what the Craftsman methodology was created to escape: in Craftsman the standing artifacts (approved SPEC.md, PLAN.md, AGENTS.md) *are* the design approval, granted once and durable, not re-extracted conversation by conversation. Likewise "questions are tasks" flattens every interaction into a skill check — reasonable for a fresh agent, hostile to an expert user asking "what does this flag do." The asymmetry inverts exactly when (a) the user is expert and the artifacts are standing, and (b) the ceremony is per-interaction rather than per-decision.

**Craftsman's structural answer** — the `craftsman.toml` applicability gate, scale routing (bug → fix, small change → quick, new behavior → spec), and the quick gear — buys precision. The honest converse: precision costs recall, and our current descriptions have measurable false-negative risk. Users report symptoms, not categories: craftsman-fix fires on "fix this bug" but "tests are failing", "it crashes", and "why does X happen" match nothing in its description. craftsman-spec is the mandated entry point for new behavior, yet triggers only on spec-jargon ("draft the spec") — "add a feature" or "build X", the most common feature-intent phrasings in existence, appear in *no* Craftsman description. craftsman-plan misses the resume phrasings ("what's left", "where were we"); craftsman-review misses the single most common request phrase in software ("code review") and "feedback". None of our descriptions uses a timing anchor or a self-state trigger — the two strongest tools in Superpowers' kit.

## Trigger-Phrase Gap Matrix

Symptom/timing triggers our descriptions lack, mined from Superpowers' phrasings, within our constraints (≤600 chars, third person, triggers front-loaded, cross-routing negatives only, applicability gate preserved):

| Skill | Missing triggers | Source pattern |
|---|---|---|
| craftsman-fix | "tests are failing", "it crashes", "why does X happen", behavior-surprise phrasing; timing anchor "before proposing any fix" | systematic-debugging |
| craftsman-implement | "continue"; self-state anchor "before claiming a batch done" (routes the completion moment to the boundary gear) | verification-before-completion |
| craftsman-spec | "add a feature", "build X", "I want it to…", "what should this do" — the feature-intent phrasings that must land on spec, not implement | brainstorming's "creating features, adding functionality" recall, minus its MUST framing |
| craftsman-plan | "what's left", "where were we", "break this down" | executing-plans/writing-plans resume vocabulary |
| craftsman-review | "code review", "feedback", "critique this" | requesting/receiving-code-review |
| craftsman-init | none — deliberately | destructive gears are never reached by near-miss inference; explicit-name triggers only (Shaping Rooms rule) |

## Proposed Revised Descriptions

Proposals only — skill files unchanged. Each ≤600 chars, verified by count.

**craftsman-fix** (592 chars) — *diff: adds symptom triggers and the "before proposing any fix" timing anchor; compresses gear detail to pay for it.*

> Craftsman bug fixing — diagnose before proposing any fix. Use when tests fail, something crashes, or behavior surprises: 'fix this bug', 'tests are failing', 'it crashes', 'why is this failing', 'why does X happen', 'scenario went red', 'regression'. Gears: diagnose (reproduce, isolate, report — the mandatory default), fix (root-cause test + minimal fix, one commit), improve (separate refactor commit). Small non-bug changes: craftsman-implement quick; new behavior: craftsman-spec. Applies only inside a Craftsman project (craftsman.toml present); otherwise offer craftsman-init and stop.

**craftsman-spec** (590 chars) — *diff: claims the feature-intent phrasings ("add a feature", "build X", "I want it to…", "what should this do") so new behavior lands on spec, not implement; "new behavior starts here" states the routing rule as identity.*

> Craftsman spec — new behavior starts here: official docs become Gherkin SPEC.md scenarios the human approves. Use for 'add a feature', 'build X', 'I want it to…', 'what should this do', 'draft the spec', 'spec this feature', 'the requirement changed'. Gears: draft (docs + example mapping — the default), delta (ADDED/MODIFIED/REMOVED change specs), recover (characterization tests pin existing behavior). Batching: craftsman-plan; building: craftsman-implement; bugs: craftsman-fix. Applies only inside a Craftsman project (craftsman.toml present); otherwise offer craftsman-init and stop.

**craftsman-implement** (599 chars) — *diff: adds "continue" and the self-state anchor "before claiming a batch done", pointing the completion moment at the boundary gear — Superpowers' verification trigger in Craftsman form.*

> Craftsman execution — turns red scenarios green at production grade. Use for 'implement', 'next batch', 'continue', 'quick change', 'small tweak', 'run the boundary', and before claiming a batch done. Gears: batch (the current PLAN.md batch — the default), boundary (batch end: gates, gap check, learnings, ledger commit, stop), finish (all green: QA, ADRs, final commit), quick (small scoped changes; gates and Verified-by mandatory). Bugs: craftsman-fix; new behavior: craftsman-spec first. Applies only inside a Craftsman project (craftsman.toml present); otherwise offer craftsman-init and stop.

**craftsman-plan** (592 chars) — *diff: adds the resume/status phrasings "what's left", "where were we", "break this down".*

> Craftsman planning — batches SPEC.md scenarios into a lean PLAN.md the work follows. Use for 'plan the batches', 'what's next', 'what's left', 'where were we', 'break this down', 'update the plan', 'replan'. Gears: batch (2–4 related scenarios, mechanical success lines — the default), revise (replan from what the last batch taught), gap (do remaining batches cover all red scenarios). Think in the harness's plan mode; PLAN.md is the durable artifact. Execution: craftsman-implement. Applies only inside a Craftsman project (craftsman.toml present); otherwise offer craftsman-init and stop.

**craftsman-review** (594 chars) — *diff: adds "code review", "feedback on this", "critique this" — the everyday request vocabulary the current description misses.*

> Craftsman review — advisory judgment, never a gate. Use for 'review this', 'code review', 'feedback on this', 'critique this', 'is this code good'. Gears: quality (architecture, naming, complexity against the AGENTS.md bar; gate output as evidence — the default), design (front-end and API taste; defers to Impeccable and Apple's skills when installed). Whether code works is craftsman verify's exit code, never this skill's opinion. Spec questions: craftsman-spec; fixes: craftsman-fix. Applies only inside a Craftsman project (craftsman.toml present); otherwise offer craftsman-init and stop.

craftsman-init stays as-is: destructive, explicit-name-triggered by design; widening its recall would violate the near-miss rule. Family total after revision ≈ 3.6k chars — still comfortable in Codex's 8k and Claude Code's ~16k catalogs.

## Beyond Descriptions

**Adopt: a red-flags/rationalization table in `craftsman-conventions.md`.** Superpowers' single best in-body invention, applied to *our* failure modes — the "this thought means stop" pattern. Since every skill reads the conventions file once per session, one table covers the family for ~120 tokens:

| Thought | Reality |
|---|---|
| "The gate is probably fine to skip this once" | Gates are the methodology. A red gate blocks the boundary — no exceptions the CLI doesn't grant. |
| "I can see the code is correct without running verify" | Green is an exit code, never a reading. Run `craftsman verify`. |
| "This quick change doesn't need the commit gate" | quick skips ceremony, never gates. `check-all --changed` + `Verified-by:` always. |
| "The scenario is basically green" | Basically green is red. Exit code 0 or it isn't done. |
| "I'll write the root-cause test after the fix" | After the fix it proves nothing. Failing test first. |
| "This rejected approach will work this time" | The ledger recorded why it failed. Warn the human and confirm before retrying. |
| "The plan is close enough, no need to revise" | A stale plan compounds. Route to craftsman-plan revise at the boundary. |
| "I know this API from training" | No source, no code. Fetch via `craftsman docs`. |

**Adopt: announce-at-start.** One line on activation — "Using craftsman-fix (diagnose) — reproducing the failure first." Zero infrastructure, agent-agnostic, gives the human a routing check *before* wrong-skill work accumulates, and (Superpowers' insight) the announcement is a public commitment that measurably improves the model's own adherence.

**Reject: hook-based skill mandate.** Craftsman uses hooks where harnesses have them for *gate enforcement* — deterministic pre-commit/pre-push checks — never for injecting prose mandates. Three reasons: hooks are harness-specific, so a trigger strategy resting on them breaks the agent-agnostic contract on Codex, Gemini CLI, opencode and the rest; the ~600-token-per-session mandate duplicates what the applicability gate and AGENTS.md (the Instructions leg, read by every harness) already do inside a Craftsman project; and where a mandate *must* hold, Craftsman's answer is stronger than exhortation — `craftsman commit` refuses, CI refuses. Enforcement lives in exit codes, not in `<EXTREMELY_IMPORTANT>` tags.

**Adopt: e2e tests of the methodology (dogfood phase).** Superpowers ships `tests/` running real harness sessions per platform, plus writing-skills' RED→GREEN doctrine: baseline an agent without the skill, document its exact rationalizations, verify the skill closes them. Craftsman's equivalent for the dogfood phase: a trigger-test harness that runs each query in Appendix A headlessly (`claude -p`, `codex exec`) in two fixture repos — one with `craftsman.toml`, one without — parses the transcript for which skill fired first, and asserts (1) the expected skill fired, (2) no must-not skill fired, (3) outside the Craftsman fixture, any craftsman skill that fires only offers init and stops. The same harness later carries pressure tests for the conventions table ("we're in a hurry, just skip verify this once"). The matrix below is the test plan.

## What Craftsman Should Adopt

1. **Symptom + timing-anchor description craft** — users say "tests are failing", not "invoke bug procedure"; the five revised descriptions above.
2. **The self-state trigger** — "before claiming a batch done" aimed at the boundary gear; the completion moment is where agents lie to themselves.
3. **The rationalization table**, once, in the conventions file — Superpowers' best idea at 1/10th its token cost.
4. **Announce-at-start** — one line, every activation.
5. **Trigger tests as e2e tests of the methodology** — the Appendix A matrix run headlessly per harness, RED→GREEN style.

## What NOT to Adopt

- **The 1% rule and universal quantifiers** — recall-maximalism whose cost asymmetry inverts for expert users with standing artifacts; Craftsman's scale routing is the calibrated replacement.
- **SessionStart prose injection** — harness-specific, per-session token rent, redundant with the applicability gate; hooks stay reserved for deterministic gate enforcement.
- **HARD-GATE design approval per interaction** — approval in Craftsman attaches to durable artifacts (SPEC.md, PLAN.md), granted once, not re-extracted every conversation.
- **Second-person MUST descriptions** — spec says third person; the applicability gate plus quoted triggers achieve the routing without the bark.
- **Behavioral negatives in descriptions** — already doctrine: cross-routing negatives only; behavior belongs in the body.
- **Per-skill rationalization tables** — one shared table in conventions; six copies would be Superpowers' token bill without its excuse.

## Conclusion

Superpowers' maturity is real but lopsided: world-class *wording* (symptom triggers, timing anchors, self-state triggers, preemptive rationalization capture) welded to a maximalist *policy* (1% rule, universal MUST, session injection) that taxes exactly the expert-with-artifacts context Craftsman serves. The harvest is to take the wording and leave the policy: five revised descriptions close our real false-negative gaps for ~400 extra catalog characters; one rationalization table in the conventions file ports the compliance pattern at a fraction of the cost; announce-at-start ports for free; and the 50-query matrix below turns trigger behavior from hope into a regression suite the dogfood phase can run on every harness. Superpowers can be removed; its lessons are now load-bearing here.

## Appendix A: Dogfood Trigger-Test Matrix

Fixture: repo with `craftsman.toml` (rows 1–46); rows 47–50 run in a bare repo. Pass = expected skill fires first; no must-not skill fires.

| # | Query | Should fire | Must NOT fire |
|---|---|---|---|
| 1 | "fix this bug" | fix | implement |
| 2 | "tests are failing" | fix | implement, review |
| 3 | "it crashes when I open settings" | fix | — |
| 4 | "why does the export produce an empty file?" | fix | review |
| 5 | "we have a regression since yesterday" | fix | — |
| 6 | "the completion scenario went red" | fix | spec |
| 7 | "this worked before the last commit" | fix | — |
| 8 | "rename this variable everywhere" | implement (quick) | fix, spec |
| 9 | "the requirement changed: exports are CSV now" | spec (delta) | fix |
| 10 | "is this error handling well designed?" | review | fix |
| 11 | "implement the next batch" | implement | plan |
| 12 | "continue" (mid-batch session) | implement | init |
| 13 | "run the boundary" | implement | review |
| 14 | "quick change: bump the default timeout" | implement (quick) | spec |
| 15 | "small tweak: rename the CLI flag" | implement (quick) | spec |
| 16 | "everything's green — wrap up" | implement (finish) | review |
| 17 | "add a feature: dark mode" | spec | implement |
| 18 | "fix the failing login test" | fix | implement |
| 19 | "what's in the next batch?" | plan | implement |
| 20 | "add a feature: users can archive projects" | spec | implement |
| 21 | "I want it to retry failed uploads" | spec | implement |
| 22 | "what should happen when the token expires?" | spec | plan |
| 23 | "draft the spec for the import flow" | spec | — |
| 24 | "the upload limit is now 10MB" | spec (delta) | implement |
| 25 | "spec what the legacy parser does today" | spec (recover) | init |
| 26 | "write a unit test for this helper" | implement | spec |
| 27 | "plan how we build the archive feature" | plan | spec |
| 28 | "why is this scenario failing?" | fix | spec |
| 29 | "plan the batches" | plan | — |
| 30 | "what's next?" | plan | implement |
| 31 | "what's left before we're done?" | plan | implement |
| 32 | "where were we?" (fresh session, mid-project) | plan | init |
| 33 | "break this spec down into batches" | plan | spec |
| 34 | "last batch changed things — update the plan" | plan (revise) | implement |
| 35 | "review this diff" | review | fix |
| 36 | "code review please" | review | — |
| 37 | "feedback on the error handling?" | review | fix |
| 38 | "critique this architecture" | review | plan |
| 39 | "is this good enough to ship?" | review | implement |
| 40 | "does it work?" | none (answer: `craftsman verify`) | review |
| 41 | "fix the issues the review found" | fix / implement (quick) | review |
| 42 | "set up craftsman in this repo" | init | — |
| 43 | "adopt craftsman here" | init (adopt) | — |
| 44 | "upgrade craftsman" | init (upgrade) | — |
| 45 | "set up the project" (no craftsman mention) | none | init |
| 46 | "initialize a git repo" | none | init |
| 47 | (bare repo) "fix this bug" | fix → offers init, stops | any gear executing |
| 48 | (bare repo) "add a feature: dark mode" | spec → offers init, stops | any gear executing |
| 49 | (bare repo) "review this diff" | review → offers init, stops | any gear executing |
| 50 | (bare repo) "what's the capital of France?" | none | all craftsman skills |
