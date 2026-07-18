# SPEC delta — dogfood harvest: verdict-path fixes, environment honesty, import, qa gates

APPROVED by the human 2026-07-18. ADDED scenarios against
current SPEC.md truth, drafted from the craftsman-web dogfood ledger
(`../craftsman-web/docs/dogfood/ledger.md`) and ADR-006. Batches 13–14
scenarios pin defect fixes and can be wired as soon as this delta is
approved; Batch 15–16 scenarios additionally wait on ADR-006 approval.
Merged into SPEC.md only at each batch boundary, once green. Until then
the executed spec stays intact.

## Batch 13 — verdict-path fixes

*(merged into SPEC.md at the Batch 13 boundary, 2026-07-18)*

## Batch 14 — environment honesty

*(merged into SPEC.md at the Batch 14 boundary, 2026-07-18; "the output names
the missing tool" was concretized to the existing contains-step with the
fixture's tool name, and the second scenario's command corrected from the
nonexistent `gate health` surface to `health`. Consequential MODIFIED
scenario, flagged for human review: "Init scaffolds a project that doctor
accepts" gained the Given "the scaffold's pinned gate tools are installed on
this machine" and its count assertion moved 5/5 → 6/6 — doctor grew the
gate-tools check, and a fresh scaffold's doctor verdict now honestly depends
on those tools being present.)*

## Batch 15 — the import gear

*(ADR-006 accepted by the human 2026-07-18; merged into SPEC.md at the
Batch 15 boundary the same day. One mechanical concretization: "the audit
report lists the health finding" became the contains-assertion on the
finding's rule name, max-function-lines.)*

## Batch 16 — qa command gates (blocked on ADR-006)

```gherkin
Scenario: A declared qa gate runs inside check-all
  Given a craftsman project declaring a qa gate whose command succeeds
  When I run craftsman with "check-all"
  Then the exit code is 0
  And the output names the qa gate

Scenario: A red qa gate blocks commit
  Given a craftsman project declaring a qa gate whose command fails
  When I run craftsman commit for a staged change
  Then the exit code is 1
  And no commit was created

Scenario: A qa gate whose command is missing refuses loudly
  Given a craftsman project declaring a qa gate whose command does not exist
  When I run craftsman with "check-all"
  Then the exit code is 3
  And the output names the qa gate command
```
