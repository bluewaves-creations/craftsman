# SPEC delta — Batch 12: pinned behaviors worth promising

PROPOSED 2026-07-18 — awaiting human approval. The Batch 12 gap closure
pinned ten behaviors with tests; this delta proposes the subset that
deserves a standing promise in SPEC.md. The rest stay guarded by their
cargo tests only (adopt phase mechanics, orchestration note wording,
trailer rendering) — and GAP-R10's record-replacement is deliberately NOT
promised: whether filtered runs should merge per scenario is an open
design decision, anchored by its pin until decided. Wiring starts only
after approval; merge at the wiring boundary.

```gherkin
Scenario: Security findings below the threshold inform but never block
  Given a craftsman project whose security scan reports one finding below the threshold
  When I run craftsman with "security"
  Then the exit code is 0
  And the output contains "informational"

Scenario: A broken security scanner is an orchestrator error
  Given a craftsman project whose security scanner exits with an unexpected code
  When I run craftsman with "security"
  Then the exit code is 3
  And the output names the broken scanner

Scenario: An impact selection that computes empty passes with a loud note
  Given a scaffolded rust project with a recorded green verify run and a clean tree
  When I run craftsman with "verify --impact"
  Then the exit code is 0
  And the output contains "nothing to run"

Scenario: A second docs get on an objects-inv page is served offline
  Given a synced objects-inv library whose page was fetched on demand once
  And the page's source has since disappeared
  When I run craftsman docs get for that page
  Then the exit code is 0
  And the output contains the page's content
```
