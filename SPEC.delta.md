# SPEC delta — update release channel (Batch 10)

Approved by the human 2026-07-18. ADDED scenarios against current SPEC.md truth;
merged into SPEC.md by the Batch 10 boundary when the implementing work
completes and they run green. Until then the executed spec stays intact.

```gherkin
Scenario: Update without an install receipt explains the reinstall path
  Given a home directory with no craftsman install receipt
  When I run craftsman with "update"
  Then the exit code is 0
  And the output names the current version
  And the output contains "install.sh"

Scenario: Update refreshes the installed skills from the binary
  Given a home directory with an outdated craftsman skill installed
  When I run craftsman with "update"
  Then the exit code is 0
  And the installed skill matches the binary's embedded copy

Scenario: Update with an unreachable release channel fails loudly
  Given a home directory with a craftsman install receipt for an unreachable release source
  When I run craftsman with "update"
  Then the exit code is 1
  And the output names the release channel
  And the output does not claim success

@requires-network
Scenario: Update self-updates to the latest release
  Given craftsman was installed from a GitHub release older than the latest
  When I run craftsman with "update"
  Then the exit code is 0
  And the reported version afterwards equals the latest release version
```
