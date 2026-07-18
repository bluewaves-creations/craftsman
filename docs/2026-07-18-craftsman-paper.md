# Craftsman: The Machine Says Pass

**A paper on why the future of AI-assisted software belongs to the people who refuse to lower the bar.**

*July 2026*

---

## Here's to the skeptics

Here's to the ones who don't believe the demo. The ones who watch an AI agent announce "All tests passing! ✅" and ask to see the exit code. The ones who know that a model can hallucinate *PASS* just as fluently as it hallucinates working code. The misfits of the vibe-coding era — the people who love what agents can do and refuse, absolutely refuse, to let an agent grade its own homework.

Craftsman was built for them. And by them.

It is not a framework. It's a rebellion against a very specific absurdity at the heart of today's AI coding tools: **we are asking the same kind of system that writes the code to tell us whether the code works.** Turtles all the way down. Craftsman's answer is almost insultingly simple:

> **Run the tests. Read the exit code. Never ask a model — including yourself — whether code works.**

Everything else in the system follows from that one refusal.

---

## The problem nobody wants to say out loud

AI agents have made software cheap to *produce* and expensive to *trust*.

The industry's own data says so. DORA's 2025 study of five thousand engineers found that AI amplifies both throughput *and* instability — you ship faster and break faster, in equal measure. Roughly 45% of unprompted AI code samples fail basic security tests. Studies of long agent sessions show verbosity creeping up in 90% of trajectories and architecture quietly eroding in 80%. And the most popular "fix" — having a second LLM review the first LLM's work — turns out to recognize correct code only 52–78% of the time, with all the reviewers making the *same* mistakes, because they're all drawn from the same distribution. A jury of clones.

The frameworks that sprang up around this problem — and some are genuinely impressive, with hundreds of thousands of stars — mostly took one approach: **ceremony**. Elaborate multi-agent rituals, brainstorming phases, review agents, plans the size of novels. They exist, in effect, to impose discipline on developers who would otherwise let agents run wild.

Craftsman starts from the opposite premise, and this is its founding heresy:

> **If you already have discipline, you don't need a framework to simulate it.**

What a disciplined developer needs is much smaller and much harder: a clean separation of responsibilities, mechanical verification you can trust, and artifacts that accumulate project intelligence across sessions without burning a fortune in tokens to re-derive it every morning.

That's it. That's the whole product.

---

## The big idea: three actors, one truth

Craftsman divides all of software development among three actors, and gives each one exactly one kind of authority. No overlaps. No committee decisions.

**The human owns taste.** Vision, architecture, the quality bar, what the product should *be*. This lives in one file — AGENTS.md — capped at a hundred lines, written by a person, never generated. (The research here is delicious: machine-generated context files actually make agents *worse* — about 3% lower success at 20% higher cost. A minimal, human-written one helps. Less prose, more precision.)

**The agent owns literacy.** Reading official documentation, drafting specifications, writing plans, writing code. The agent is the librarian: brilliant, tireless, fast — and never, ever the judge. The house rule is four words: *no source, no code*. If a library isn't in the project's declared documentation table, the agent stops and asks rather than "remembering" an API that may have changed or never existed.

**The machine owns verdicts.** A deterministic command-line tool called `craftsman` runs the tests, runs the gates, and answers with an exit code. Zero means pass. Anything else means it isn't done. The verdict costs zero tokens, takes zero interpretation, and cannot be sweet-talked.

The internal shorthand for this is blunt: *"Does it work?" is the machine's question. "Is it good?" is yours.*

And the evidence backs the split. The single strongest result in Craftsman's research corpus: giving agents mechanical, impact-mapped test feedback cut regressions by 70%. Meanwhile, simply *instructing* agents to "do TDD" — ritual without mechanism — made regressions **worse**. Agents improve where feedback is mechanical, contextual, and impossible to argue with. They degrade where quality depends on instructions they can reinterpret. That single finding is the whole methodology in one sentence.

---

## The spec is the test suite

Most spec-driven tools of the 2026 wave — Spec Kit, Kiro, OpenSpec, and their cousins — produce prose specifications that an LLM then *interprets* into code and, worse, interprets into a judgment about whether the code matches. One startup raised $125 million on "the spec is the source code" and quietly retreated when it discovered the obvious: a non-deterministic compiler produces different code from the same spec every run.

Craftsman takes the other side of that divide, and plants a flag on it: **specs verify code; they don't replace it.**

Behavior is written in Gherkin — plain-language *Given / When / Then* scenarios a human can read over coffee — in a single file, SPEC.md, that only the human approves. Here's the trick that changes everything: each scenario is mechanically generated into a real test in the project's own stack. The scenario isn't a description *of* the requirement. The scenario **is** the requirement, the acceptance criterion, and the executable check — one artifact, three jobs. Every scenario starts red. The work is done when the machine says they're all green. The agent cannot game a sentence that compiles.

Around that spec sits a deliberately tiny set of artifacts — two working files, not five, because the great lesson of the spec-driven wave was that artifact count is the enemy:

- **SPEC.md** — what the software must do. Human-owned. Frozen during implementation.
- **PLAN.md** — the order of attack: small batches of two to four related scenarios, each ending in a mechanical success line. The spec doesn't move; the plan does.
- **The git ledger** — every commit carries structured trailers: which scenarios it touched, what was *Learned*, what was tried and *Rejected* and why. Project memory that costs zero tokens at rest and is queryable forever. Before proposing any architectural approach, the agent must check the record of rejected ones — so the system never confidently re-attempts last month's failure.
- **Decision records** — the long-term "why," consolidated with human sign-off.

A fresh agent with these files knows everything a returning agent would know, with zero warm-up. Memory as *files*, not vibes.

---

## Discipline you cannot forge

Most methodologies enforce discipline the way a gym poster enforces fitness. Craftsman enforces it the way gravity does.

**The unforgeable trailer.** Every ledger commit that passed verification carries a `Verified-by:` trailer — and the CLI is the *only* thing that can write it. There's no flag to set it. The code actively scans for and rejects a hand-smuggled one. If any quality gate is red, `craftsman commit` simply refuses. Convention can be ignored; the commit gate cannot. Green gates become unforgeable history.

**Nine gates, one ratchet.** Verification, lint, architecture rules, security scanning, code health, mutation testing, performance, accessibility, visual regression — each independently set to off, baseline, or strict. Baseline mode is the genius move for real codebases: it snapshots today's existing debt and fails only on *new* violations, then automatically ratchets down as debt is paid. Improvement gets locked in; backsliding gets blocked. (A gate flipped to strict against twenty years of legacy code is a gate everyone learns to ignore. Craftsman's gates are designed to be *believed*.) One gate never bends: the spec itself. Baselines never apply to behavior.

**Honesty about failure, by design.** In most tools, a filter that matches nothing exits zero — silence indistinguishable from success. In Craftsman, "matched nothing" is its own exit code. A broken scanner is never a green gate. An unconfigured check is an error, never a silent skip. The system refuses to let *absence of bad news* impersonate *good news*.

**Rules for the agent's own worst instincts.** A bug gets a diagnosis before it gets a fix — "a diagnosis without reproduction is a hypothesis wearing a costume." Every fix ships with a failing-first test that proves the root cause. Fix and refactor never share a commit. Three failed attempts means stop, write down what you learned, and report — never lower the bar to get to green. And the red-flags list names the little lies out loud: *"basically green" is red. Exit code 0 or it isn't done.*

This is the deeper point about discipline: Craftsman doesn't ask anyone to *be* disciplined in the moment. It moves discipline out of willpower and into mechanism, where it can't decay. The governing principle: **prose rules decay; gates don't.** Every rule that can be mechanized becomes a gate. The prose keeps only what machines can't hold — taste and vision.

---

## Token efficiency: structural, not incremental

Everyone else optimizes tokens by trimming prompts. Craftsman optimizes by *architecture* — three layers with completely different cost profiles, and a rule for what belongs in each:

- **Instructions** (AGENTS.md): always loaded, so kept ruthlessly small — a hundred lines, cached to a few hundred tokens after the first turn. Compare that to frameworks that re-derive project context conversationally each session at ~20,000 tokens a pop. That's a 100× reduction for the same function.
- **Skills**: knowledge loaded on demand, gear by gear, released when done. A shared conventions file travels byte-identical inside all six skills and is read once per session — no skill re-teaches the rules.
- **The CLI**: deterministic action at **zero prompt cost**. This is the structural coup, and it's aimed straight at tool-protocol architectures: adding ten commands to a CLI adds *nothing* to any prompt, while adding ten MCP tools adds ten schema blocks to *every* prompt. Fifty commands cost the same as five. Verification — the thing done most often — costs zero tokens, every single time.

Two more heresies complete the economics. **Compress, don't spawn:** implementation stays in one continuous context, and at each batch boundary the agent extracts durable learnings to disk instead of paying the ~14,000-token launch tax popular frameworks charge per sub-agent. **Failure loops are the real token sink:** the most expensive thing an agent does is thrash — attempt, fail vaguely, retry blind. A methodology whose feedback is sharp and mechanical fails less, so it's cheaper even when a single turn costs more. Craftsman measures itself in the only currency that matters: **tokens per green scenario.**

---

## The CLI: an incorruptible referee

At the center sits ~21,000 lines of Rust with three promises engraved in the architecture:

**No LLM calls. Ever.** Not one, anywhere in the binary.

**No network in the verdict path.** Exactly two commands ever touch the internet — syncing documentation and updating itself — and neither can influence a pass/fail. Your verdicts work on an airplane.

**No telemetry.** Nothing phones home. Your code, your history, your business.

Around the referee, one more load-bearing rule: **single writer.** Only the CLI writes verification state, baselines, and ledger trailers. The agent judges and composes; the CLI records. That's what makes the history auditable — every green mark in the ledger was earned through one incorruptible doorway, so you can trust a repo's record the way you trust a bank statement.

The tool holds itself to its own standard, and this is where it gets beautifully recursive: **Craftsman is built with Craftsman.** The repo has its own SPEC.md; the CLI verifies itself against its own Gherkin scenarios; every commit in its history went through `craftsman commit` and carries the trailers. The strictest lint tiers run with warnings as errors. A `doctor` command proves the whole red→green loop end-to-end in a disposable project before you trust it with yours. Even the project's *plan* keeps an explicit "honest undone" register — a public list of what is *not* finished — because a system built on unforgeable verdicts doesn't get to round up about itself.

One more quiet superpower lives here: the documentation pipeline. Official docs — Rust API JSON, llms.txt indexes, Apple DocC exports, Python inventories, TypeScript type definitions — are declared once, synced, then searched entirely offline at ripgrep speed. The agent grounds every API call in the real, version-pinned documentation instead of its training-data memories. And every fetched page is treated as *data, never instructions* — a built-in defense against poisoned documentation. Hallucinated APIs, the most common failure in agentic coding, simply lose their oxygen.

---

## Plays well with everyone, kneels to no one

**Any agent.** Craftsman is deliberately harness-agnostic. Its six skills follow the open skills standard, install once into a canonical home, and adapt to Claude Code, Cursor, Codex, Gemini, Windsurf, Goose — whatever comes next. The canonical context file is AGENTS.md, the industry-neutral name; CLAUDE.md is just a symlink. Where a harness supports hooks, Craftsman wires them; where it doesn't, the unforgeable commit gate and CI enforce the same line. Crucially, it *refuses to compete* with what harnesses already do well — session memory, compaction, plan-mode UX, multi-agent orchestration all stay native. Craftsman claims only the territory a platform vendor will never claim: **your definition of done.**

**Six stacks, one contract.** Swift, Python, TypeScript, Rust, Bash — each with a first-class adapter that turns Gherkin scenarios into that stack's own native tests (Swift Testing, pytest-bdd, cucumber-js on Bun, cucumber-rs, bats) and normalizes every result into one uniform verdict. Same spec language, same exit codes, same discipline, whether you're shipping a server or a shell script.

**The Apple ecosystem, taken seriously.** This is where Craftsman goes where nearly no one else bothers. Gherkin scenarios become real Swift Testing suites. It drives `xcodebuild` against SwiftPM packages directly — no project-file archaeology — with test-identifier syntax reverse-engineered to the backtick, on real Xcode 26 and 27 toolchains, because "silently matched zero tests" was never going to be acceptable. Accessibility audits run through Apple's own XCUITest machinery as a first-class gate. And with Xcode 27's exportable Agent Skills, Craftsman composes rather than duplicates: Apple's skills own platform idiom, Craftsman owns process. Verified spec-driven development for iOS and macOS — an empty niche, now occupied.

---

## Why it wins

Craftsman's own competitive research is unsparing — it audits where the system would *lose* today (harness-native planning UX, breadth of semantic AI review) with the same rigor it audits its wins. That honesty is exactly why the wins are credible. Five things, together, that nobody else has:

1. **Machine-verdict-only pass/fail.** Every competing framework's verdict — including the most celebrated ones — is still, at bottom, an LLM's opinion. Craftsman's is an exit code.
2. **The executable spec, across six stacks.** The spec-driven wave writes prose for LLMs to interpret. Craftsman writes scenarios that machines run. No one else makes Gherkin the verified contract — least of all on Swift.
3. **Unified gate orchestration with a ratchet.** Static analysis tools do lint. Review bots do opinions. Nobody else runs static *and* runtime gates — tests through security through mutation through accessibility — under one CLI, one exit-code contract, one baseline-and-ratchet memory.
4. **Provenance you can trust.** A git ledger where green is unforgeable and failures are recorded as *Rejected:* lessons — project memory at zero token cost, and a history you can audit.
5. **Positioned on the non-absorbable side.** The bear case says frontier harnesses will absorb all of this within a year. But what they absorb is session-side convenience. What they structurally avoid is durable project-side artifacts and opinionated definitions of done — because a vendor won't declare *your* quality bar. Craftsman's one non-negotiable — *the machine says pass* — only becomes more valuable as everything else gets faster.

And underneath all five: the whole verification market is now converging on Craftsman's founding thesis — deterministic checks on AI-written code. The thesis is validated. The implementation is uncontested.

---

## Who it's for

Craftsman is for the developer who was disciplined *before* the agents arrived — and who felt the bar dropping the day they did.

It's for the senior engineer who deleted an agent's "comprehensive test suite" after noticing that none of the tests could fail. For the indie Apple developer who wants spec-driven rigor on Swift without an enterprise process. For the team lead who is done reading "everything looks good!" above a diff that doesn't compile. For the person who believes an agent writing 80% of the code is a *reason* for standards to go up, not a license for them to slide. For anyone who has ever typed the words *"are you sure the tests actually pass?"* into a chat box and felt a little sick doing it.

It is not for everyone, and it doesn't pretend to be. If you want a framework to do your thinking, to brainstorm on your behalf, to tell you your work is great — the ecosystem is full of warm options. Craftsman assumes you bring the judgment. It brings the mechanism that makes your judgment stick — lean enough to internalize, opinionated enough to prevent drift, honest to the point of rudeness.

The people who are serious enough about their craft to demand proof of it — they're the ones this was built for. Because the developers who refuse to take an agent's word for it are the ones who'll actually change what agents can be trusted to build.

**The human owns the vision. The agent does the work. The machine says pass.**

Everything else is ceremony.

---

*Craftsman is the `craftsman` CLI (Rust), six agent skills, and a small committed contract of files. Design authority: `docs/design/`. Evidence: the 22-document research corpus in `docs/research/`, with claims graded by strength. The repo eats its own cooking: every commit in its history carries a CLI-written `Verified-by:` trailer.*
