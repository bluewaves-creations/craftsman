# Import (foreign tree)

Loaded to bring a tree that arrived from *elsewhere* — a copied sibling, a forked or vendored open source project — under Craftsman (ADR-006). Import is not adopt: you did not grow this code, and it does not meet the quality bar until proven otherwise. The stance is **audit first, hide nothing**: every flaw is surfaced and explicitly disposed of; nothing is silently baselined.

This gear is destructive (it writes the contract files). State the file list and get confirmation before running anything.

## 1 — Scaffold + detect

`craftsman import --name <name> --stack <stack>` — scaffolds the contract non-destructively (existing files are `kept`, never overwritten; `.gitignore` merged) and reports detected QA commands (package scripts) as `[gates.qa]` conversion candidates. Requires git; run `git init` first on an unversioned copy, and make the tree's arrival its own first commit so the ledger records provenance.

## 2 — Audit (report-only)

`craftsman import --audit` — runs every enabled non-verify gate **forced strict** and prints the complete flaw inventory. Exit 0; findings are the report, not a verdict. Nothing is baselined. Read it with the human: these numbers are the inherited debt made visible.

## 3 — Dispose of the debt, explicitly

For each audit finding class, the human picks one of exactly two moves:

- **Remediate** (the default): a batch in PLAN.md that fixes it, ordered with the rest of the roadmap.
- **Baseline** (the exception, with a recorded reason): `craftsman gate baseline <gate>`, then flip that gate to `baseline` in craftsman.toml. The ratchet pays it down permanently.

Never baseline by reflex — that is adopt's Phase 2 for code you own. Imported code defaults to remediation.

## 4 — Convert existing QA

Map what the tree already enforces into the contract, with the human approving each mapping:

- Lint configs → the `lint` gate's tools.
- Test suites → verify adapters (stack-native BDD) or characterization harnesses via craftsman-spec `recover`.
- Residual QA commands (the detected candidates) → `[gates.qa.<name>] command = "…"` entries — they then run inside `check-all`, the commit gate, and the `Verified-by:` trailer. `verify` itself is never an external command.

## 5 — Prove the loop

Walking-skeleton scenario per stack (or a first recovered scenario), observed red → green through `craftsman verify`; first ledger commit via `craftsman commit`. From here the tree is a normal Craftsman project.
