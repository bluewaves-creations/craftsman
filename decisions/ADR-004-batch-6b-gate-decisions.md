# ADR-004: Batch 6b gate decisions — config placement, arch scope, mutation tooling

Status: accepted · Date: 2026-07-18 · Evidence: TOML semantics; live probes
of cargo-mutants 27.1.0, mutmut 2.5.1/3.6.0, and the design-doc sketch,
all on this machine.

## 1. Gate settings live in top-level tables, not under `[gates.<name>]`

The design doc sketches `[gates.arch.rules]` while `[gates]` also sets
`arch = "strict"`. That TOML is unparseable: `gates.arch` cannot be both a
string and a table. The batch instructions inherited the same shape
(`[gates.mutate] min-score`, `[gates.health]` thresholds).

**Decision:** per-gate settings are top-level tables — `[health]`,
`[mutate]`, `[arch]`, `[perf]`, `[a11y]`, `[visual]` — exactly the
precedent `[verify]` already set (mode in `[gates] verify`, settings in
`[verify.*]`). Modes stay in `[gates]`.

## 2. Arch is dependency direction only; `max-file-lines` belongs to health

The sketch put `max-file-lines = 400` inside the arch rules. File size is
an entropy/health metric (research doc: size/complexity/duplication are
the evidence-backed erosion gates), not a dependency property.
**Decision:** `craftsman arch` enforces `deny = ["A -> B"]` direction
rules only; `max-file-lines` is `[health]`'s threshold. An enabled arch
gate with zero rules refuses (exit 3) — never silent green.

## 3. Mutation tooling per stack

- **rust — cargo-mutants 27.1.0 via `cargo install --root
  ~/.craftsman/tools/cargo-mutants@<pin>`**, not a release binary:
  `cargo install` is uniform across platforms, the toolchain is already a
  hard requirement of the rust stack, and the install is cached forever.
  Observed exit codes: 0 clean · 2 missed · 3 timeout · 4 unviable.
  Verdicts parse from `mutants.out/outcomes.json` (totals + per-mutant
  `scenario.Mutant.{file, span.start.line}`, observed live). Test args
  are pinned to `-- --lib --bins`: the mutants tree is a copy of the
  package, where integration tests reading outside it (this repo's own
  SPEC.md harness) cannot run.
- **python — mutmut pinned 2.5.1, not 3.x.** mutmut 3 moved source-path
  selection into config files with no CLI override (verified live: 3.6.0
  errors without `setup.cfg [mutmut] source_paths`), so its diff-scoping
  story is weak; 2.5.1's `--paths-to-mutate` scopes to changed files
  directly (file granularity). Known limit, accepted: mutmut 2's results
  browser crashes on python ≥ 3.13 (pony ORM, verified live), so
  survivors are reported as an aggregate finding, not per line.
- **typescript — Stryker (`bunx @stryker-mutator/core@9.6.1`)** with
  `--incremental` and `--mutate <changed files>`; verdicts from the
  mutation-testing-report-schema JSON.
- **swift/bash — refused loudly** (exit 3): no production-consensus tool
  (research doc flags muter as non-consensus). A stack the gate cannot
  exercise is never reported green.

## 4. Full mutation runs refuse without consent at the parser level

`craftsman mutate --all` requires `--yes-slow` via clap's `requires` —
the refusal is a usage error (exit 2), matching the exit-code contract
("2 usage") rather than a gate verdict. Diff-scoped is the only mode
check-all ever runs.
