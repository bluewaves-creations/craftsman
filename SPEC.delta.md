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

  Scenario: Plan lint accepts a scenario that lives in the approved delta
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    And a delta file adds the scenario "Third behavior"
    And the plan assigns "Third behavior" to a batch
    When I run craftsman with "plan lint"
    Then the exit code is 0
    And the output contains "delta"

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
