# ADR-006: Entry doctrine — init, adopt, import; existing QA converts into gates

Status: Proposed · Date: 2026-07-18 · Evidence: the craftsman-web dogfood
ledger (`../craftsman-web/docs/dogfood/ledger.md`, findings 1, 3, 4b) and
the human's doctrine statement of 2026-07-18.

## Context

The first external dogfood (craftsman-web, a sibling site copied and
rebranded, then brought under Craftsman) exposed a routing hole: the tree
was materially brownfield but the request read as greenfield, so `init`
scaffolded strict-from-birth gates over code the project never grew
line-by-line. The health gate blocked on inherited findings until a manual
`gate baseline`; the always-strict verify gate was satisfied only by a
token walking skeleton while the site's real acceptance (`bun run qa`)
lived entirely outside the system. The ledger's verdict: the
greenfield/brownfield fork rested on nothing but how the human phrased the
request.

The human then set the doctrine (2026-07-18): importing an existing
codebase *from elsewhere* is a different act from adopting your own.
Imported trees — typically open source — never meet the quality bar, so
the system must **surface** their flaws, not absorb them silently; and
when an imported project already carries QA, that QA must be **converted**
into the Craftsman system cleanly, not left as a parallel authority.

## Decision

**1. Three entry gears, one per situation.**

| Gear | Situation | Debt stance |
|---|---|---|
| `init` | Greenfield: empty tree | Strict from birth — there is no debt |
| `adopt` | Your own brownfield, in place | Five-phase protocol (unchanged): observe → ledger → baseline → recover → steady |
| `import` (new) | A tree that arrived from elsewhere: copied sibling, forked or vendored open source | Full audit first; debt is inventoried and *visibly* accepted or scheduled — never silently baselined |

**2. `init` refuses a non-empty tree.** When the target already contains
source files, `init` without `--force` exits 3 and names `adopt` and
`import`. The fork no longer rests on the human's phrasing; the CLI
detects it mechanically. `--force` remains the human's override.

**3. The `import` pipeline.**

1. *Detect* — stacks, existing QA commands (package scripts, Makefile
   targets, CI workflows), test suites, lint configs. Reported, not acted on.
2. *Scaffold* — the full contract (craftsman.toml, AGENTS.md skeleton,
   harness hooks, `.craftsman/`), non-destructively, exactly like `init`'s
   merge behavior.
3. *Audit* — run every gate that can run, in observe mode, and emit the
   complete flaw inventory (per-gate findings, counts, JSON + human
   report). Nothing is hidden; this is the "solid new system that spots
   the existing flaws".
4. *Convert QA* — map what was detected into the Craftsman contract:
   existing lint configs → the `lint` gate; existing test suites → verify
   adapters or characterization harnesses (via craftsman-spec recover);
   residual project QA commands → declared `[gates.qa]` command gates (§5).
5. *Dispose of debt, explicitly* — the human either accepts a finding into
   a recorded baseline (committed, inspectable, ratcheted down) or routes
   it to a remediation batch in PLAN.md. The default is remediation;
   baseline is the exception that carries a reason.

**4. `verify` stays always-strict BDD.** An external command never
satisfies the verify gate. **Rejected:** the external-command verify
adapter floated in the craftsman-web ledger — it would dissolve the
spec-as-test-suite invariant that the whole methodology stands on. A
content site's honest arrangement is a real (if small) executable spec
plus its QA converted into gates.

**5. First-class QA command gates.** `[gates.qa.<name>]` declares a
project command (`command = "bun run qa:links"`) as a gate under Craftsman
orchestration: exit-code contract, runs inside `check-all` (and therefore
inside the commit gate and the `Verified-by:` trailer), refuses loudly
(exit 3) when the command is missing or undeclared. Modes: `strict | off`
in v1 — a command verdict has no findings to fingerprint, so `baseline`
does not apply. The verdict path may execute local project tooling (as
`verify` already does); the no-network rule continues to bind the CLI
itself, and the no-install rule (AGENTS.md, from ledger finding 2) binds
every adapter and gate runner.

## Consequences

- Implementation plan Batches 15 (import gear) and 16 (qa gates) — blocked
  on approval of this ADR and of their SPEC.delta.md scenarios.
- The craftsman-init skill grows `import` routing signals ("copied from",
  "fork", "bring this existing repo under craftsman") and an import gear;
  destructive-gear confirmation rules apply unchanged.
- craftsman-web re-enters through `craftsman import` once it ships
  (dogfood Phase D6) — the walking skeleton stays, `bun run qa` becomes
  declared qa gates, and the copied-tree health debt becomes an explicit
  register instead of a bare baseline file.
- `adopt` is untouched: in-place brownfield with your own history keeps
  the five-phase protocol.
