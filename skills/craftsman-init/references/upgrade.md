# Upgrade (upgrade)

Loaded to refresh a Craftsman installation: new CLI version, updated skills, or a reported conventions drift.

## The sequence

1. **Report first.** `craftsman update --check` (CLI + bundled skills version vs the repo's `cli-version` pin) and a conventions integrity check — if any skill's `references/craftsman-conventions.md` differs from the canonical copy, say which and show the diff. Never patch a drifted copy by hand; the refresh replaces it wholesale.

2. **Confirm scope.** What will change: the CLI binary, the installed skill files (`craftsman setup` refresh), the `cli-version` pin in craftsman.toml. User project files — AGENTS.md, SPEC.md, PLAN.md, decisions/, baselines — are **never** touched by an upgrade. State this explicitly, then get the yes.

3. **Execute.** `craftsman update`, then `craftsman setup` (attribution-checked: it only replaces what it installed — foreign files in the skills directories are reported, left alone). Update the `cli-version` pin.

4. **Re-prove.** `craftsman doctor`. A changed CLI that hasn't re-proven the loop is an assumption, not an upgrade. If a new CLI version added gates or changed adapter behavior, `craftsman check-all` once and report any newly-red gate to the human — new red after upgrade is a conversation, not a silent fix.

5. **Ledger.** `craftsman commit` — `chore: upgrade craftsman <old> → <new>`, body listing what changed.

## Never

- Never touch user project content during an upgrade.
- Never resolve conventions drift by editing the drifted copy.
- Never finish without a passing `craftsman doctor`.
