# Greenfield Bootstrap (new)

Loaded to bring an empty or new repository under Craftsman. Outcome: a repo where `craftsman doctor` passes and the first spec-draft session can start immediately.

## The sequence

1. **Confirm scope.** List what will be created: `AGENTS.md`, `craftsman.toml`, `.craftsman/`, harness wiring files, a walking-skeleton test target. Get a yes. Not a git repo → offer `git init` first; the ledger needs git.

2. **The AGENTS.md interview.** Short, human-attested, one topic at a time:
   - purpose and users (2–3 sentences),
   - tech stack and the commands that build/test/run — **execute each command as it's given; only commands that ran successfully go in**,
   - hard constraints (the non-negotiables: "no `any`", "structured concurrency only") — for each, check whether a gate can enforce it; if yes, it becomes a craftsman.toml rule and AGENTS.md just points there,
   - taste: what good looks like, with one concrete code example per convention (show-don't-tell),
   - the Documentation Sources table: every library/platform the project will code against, its source (`mcp | llms.txt | docc | docsrs-json | objects.inv | dts | file | context7`), pinned version, verify gate. Closing line: "Unlisted library → STOP and ask."

   Budget: ≤100 lines of rules (craftsman.toml `budgets.tokens.agents-md-lines`). If the interview produces more, the overflow is either a gate rule (move it) or not load-bearing (cut it).

3. **craftsman.toml.** `craftsman init` proposes stack auto-detection; human confirms stacks, enabled gates (greenfield default: everything relevant to the stack, all `strict`), budgets, tool pins.

4. **Verify harness.** Wire the stack's runner (pytest-bdd scaffold, cucumber-js config, `spec gen` target for Swift/bash). Create one trivial walking-skeleton scenario in SPEC.md (human approves even this one).

5. **Harness wiring.** Hooks where supported (Claude Code, Cursor): gate-enforcement templates. `CLAUDE.md` as a symlink to `AGENTS.md` — the single tolerated harness artifact. Apple project: probe `xcrun agent skills export`; on success install Apple's skills and record the delegation in AGENTS.md.

6. **Prove the loop.** `craftsman doctor`: config valid, tools resolve, and the walking-skeleton scenario observed red → implemented → green through `craftsman verify`. A bootstrap that never saw red has proven nothing.

7. **First ledger commit.** `craftsman commit` — `chore: bring repo under craftsman` with the created files listed.

## Never

- Never fill AGENTS.md sections the human didn't answer — a missing section beats an invented one.
- Never enable a gate you didn't verify runs on this machine.
- Never skip the red observation in step 6.
