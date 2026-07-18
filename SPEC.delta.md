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

```gherkin
Scenario: Doctor reports a pinned gate tool missing from the machine
  Given a craftsman project that pins a gate tool that does not exist on this machine
  When I run craftsman with "doctor"
  Then the exit code is 1
  And the output names the missing tool

Scenario: A baseline-mode refusal names the baseline command
  Given a craftsman project with a baseline-mode health gate, no recorded baseline, and an existing finding
  When I run craftsman with "gate health"
  Then the exit code is 1
  And the output contains "craftsman gate baseline health"
```

## Batch 15 — the import gear (blocked on ADR-006)

```gherkin
Scenario: Init refuses a non-empty tree and names the import path
  Given a git repository that already contains source files
  When I run craftsman with "init --name legacy --stack rust"
  Then the exit code is 3
  And the output contains "import"
  And no scaffold files were written

Scenario: Import scaffolds the contract without destroying existing files
  Given a git repository that already contains source files
  When I run craftsman with "import --name legacy --stack rust"
  Then the exit code is 0
  And the existing source files are unchanged
  And the scaffold includes "craftsman.toml"

Scenario: Import audits the enabled gates and reports the flaw inventory
  Given an imported project whose existing code carries a health finding
  When I run craftsman with "import --audit"
  Then the exit code is 0
  And the audit report lists the health finding
  And no baseline was recorded

Scenario: Import detects existing QA commands as conversion candidates
  Given a git repository with a package script named "qa"
  When I run craftsman with "import --name legacy --stack typescript"
  Then the exit code is 0
  And the output lists "qa" as a conversion candidate
```

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
