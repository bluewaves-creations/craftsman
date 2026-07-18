Feature: Craftsman CLI core — delta

  # ADDED scenarios — delta mediation + boundary observability (approved
  # scope 2026-07-19: buckets A1, A2, A3). This file merges into SPEC.md
  # only at the implementing batch's boundary — and that merge should be
  # the first live run of `spec merge-delta` itself.
  #
  # Design decisions embedded here:
  # - "ledger commits" = commits carrying a Verified-by trailer; hand or
  #   docs commits do not count toward boundary distance.
  # - The session line is pure visibility: it never warns, never blocks,
  #   and appears on both `spec status` and `craftsman commit` (stderr).
  # - `spec merge-delta` writes SPEC.md (mediated single-writer) but
  #   never commits; the head stays where it was.

  Scenario: Spec lint checks a delta file without admitting it
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    And a delta file adds the scenario "Third behavior"
    When I run craftsman with "spec lint --delta"
    Then the exit code is 0
    And the output contains "Third behavior"
    When I run craftsman with "spec status --json"
    Then stdout is valid JSON listing 2 scenarios

  Scenario: Spec lint delta rejects a name colliding with the executed spec
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    And a delta file adds the scenario "First behavior"
    When I run craftsman with "spec lint --delta"
    Then the exit code is 1
    And the output contains "collides"

  Scenario: Spec lint delta without a delta file is an empty selection
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    When I run craftsman with "spec lint --delta"
    Then the exit code is 4
    And the output contains "SPEC.delta.md"

  Scenario: Merge-delta folds approved scenarios into the spec and removes the delta
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    And a delta file adds the scenario "Third behavior"
    When I run craftsman with "spec merge-delta"
    Then the exit code is 0
    And the delta file is gone
    And the repository head is unchanged
    When I run craftsman with "spec status --json"
    Then stdout is valid JSON listing 3 scenarios

  Scenario: Merge-delta refuses a delta that fails lint
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    And a delta file adds the scenario "First behavior"
    When I run craftsman with "spec merge-delta"
    Then the exit code is 1
    And the delta file still exists
    When I run craftsman with "spec status --json"
    Then stdout is valid JSON listing 2 scenarios

  Scenario: Merge-delta without a delta file is an empty selection
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    When I run craftsman with "spec merge-delta"
    Then the exit code is 4
    And the output contains "SPEC.delta.md"

  Scenario: An extract resets the boundary distance to zero
    Given a craftsman project where an extract just ran at the current head
    When I run craftsman with "spec status"
    Then the exit code is 0
    And the output contains "0 ledger commits since last extract"

  Scenario: Ledger commits after the last extract are counted visibly
    Given a craftsman project where 2 ledger commits landed after the last extract
    When I run craftsman with "spec status"
    Then the exit code is 0
    And the output contains "2 ledger commits since last extract"

  Scenario: Commit reports the distance to the last extract
    Given a craftsman project whose gates are all green
    And an extract ran at the current head
    And a file is staged
    When I run craftsman commit with type "chore" and message "observe boundary distance"
    Then the exit code is 0
    And the output contains "1 ledger commit since last extract"

  Scenario: Spec status is explicit when no extract has ever run
    Given a craftsman project where no extract has ever run
    When I run craftsman with "spec status"
    Then the exit code is 0
    And the output contains "no extract recorded"
