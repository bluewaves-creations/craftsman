Feature: Craftsman CLI core

  The craftsman binary is the machine actor of the Craftsman Dev triad:
  every verdict is an exit code, never a judgment call. These scenarios
  drive the compiled binary against disposable fixture projects and are
  run by craftsman itself via the cucumber-rs harness in cli/tests/spec.rs.

  Scenario: Spec status lists every scenario in the spec
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    When I run craftsman with "spec status"
    Then the exit code is 0
    And the output contains "First behavior"
    And the output contains "Second behavior"
    And the output contains "unknown"

  Scenario: Spec status shows the last verify verdicts
    Given a scaffolded rust project with a recorded green verify run
    When I run craftsman with "spec status"
    Then the exit code is 0
    And the output contains "pass"
    And the output contains "The loop closes"

  Scenario: Spec status emits machine-readable JSON
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    When I run craftsman with "spec status --json"
    Then the exit code is 0
    And stdout is valid JSON listing 2 scenarios

  Scenario: Spec lint accepts a clean spec
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    When I run craftsman with "spec lint"
    Then the exit code is 0

  Scenario: Spec lint rejects duplicate scenario names
    Given a craftsman project whose spec has scenarios "Twice told" and "Twice told"
    When I run craftsman with "spec lint"
    Then the exit code is 1
    And the output contains "duplicate"

  Scenario: Spec lint rejects a batch tag
    Given a craftsman project whose spec has a scenario tagged "@batch-2"
    When I run craftsman with "spec lint"
    Then the exit code is 1
    And the output contains "PLAN.md"

  Scenario: Spec gen refuses when the spec has lint errors
    Given a bash-stack craftsman project whose spec has scenarios "Twice told" and "Twice told"
    When I run craftsman with "spec gen"
    Then the exit code is 1
    And the output contains "spec lint"

  Scenario: Spec gen writes a generated header
    Given a bash-stack craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    When I run craftsman with "spec gen"
    Then the exit code is 0
    And the generated bats file contains "GENERATED"

  Scenario: Spec gen never overwrites step implementations
    Given a bash-stack craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    And spec gen has run and the step template was hand-modified
    When I run craftsman with "spec gen"
    Then the exit code is 0
    And the step template still carries the hand modification

  Scenario: Verify fails loudly when the scenario filter matches nothing
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    When I run craftsman verify for the scenario "No such behavior"
    Then the exit code is 4

  Scenario: Verify runs every configured stack
    Given a craftsman project configured with stacks "rust" and "cobol"
    When I run craftsman with "verify"
    Then the exit code is 3
    And the output contains "cobol"

  Scenario: Verify reports an undefined scenario as a failure
    Given a scaffolded rust project whose spec has an unimplemented step
    When I run craftsman with "verify"
    Then the exit code is 1
    And the output contains "1 undefined"

  Scenario: Impact falls back to running everything when no map exists
    Given a scaffolded rust project that verifies green
    When I run craftsman with "verify --impact"
    Then the exit code is 0
    And the output contains "no impact map"
    And the output contains "The loop closes"

  Scenario: Verify refuses to run without a craftsman config
    Given an empty project directory
    When I run craftsman with "verify"
    Then the exit code is 3
    And the output contains "craftsman.toml"

  Scenario: Config rejects a verify gate weaker than strict
    Given a craftsman project whose config sets the verify gate to "baseline"
    When I run craftsman with "spec status"
    Then the exit code is 3
    And the output contains "strict"

  Scenario: Plan lint accepts a plan covering existing scenarios
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    And its plan assigns batch 1 the scenarios "First behavior" and "Second behavior"
    When I run craftsman with "plan lint"
    Then the exit code is 0

  Scenario: Plan lint rejects a scenario missing from the spec
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    And its plan assigns batch 1 the scenarios "First behavior" and "Ghost behavior"
    When I run craftsman with "plan lint"
    Then the exit code is 1
    And the output contains "Ghost behavior"

  Scenario: Lint reports findings with file and line
    Given a rust gate fixture with a seeded formatting finding
    When I run craftsman with "lint"
    Then the exit code is 1
    And the output contains "src/lib.rs:1"

  Scenario: Gate baseline then rerun goes green
    Given a rust gate fixture with a seeded finding and the lint gate in baseline mode
    And its lint baseline has been recorded
    When I run craftsman with "lint"
    Then the exit code is 0
    And the output contains "baselined"

  Scenario: Gate strict refuses while the baseline is nonempty
    Given a second rust gate fixture with a seeded finding and the lint gate in baseline mode
    And its lint baseline has been recorded
    When I run craftsman with "gate strict lint"
    Then the exit code is 1
    And the output contains "1 finding"

  Scenario: Check-all skips an unchanged clean gate via the cache
    Given a clean rust gate fixture under git with the lint gate strict
    When I run craftsman check-all twice
    Then the exit code is 0
    And the output contains "cache"

  Scenario: Arch rejects a denied dependency direction
    Given a craftsman project with an arch deny rule and a violating import
    When I run craftsman with "arch"
    Then the exit code is 1
    And the output contains "src/a"
    And the output contains "src/b"

  Scenario: Health flags an over-long function
    Given a craftsman project whose source has a function longer than the health limit
    When I run craftsman with "health"
    Then the exit code is 1
    And the output contains "max-function-lines"

  Scenario: Mutate refuses full runs without explicit consent
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    When I run craftsman with "mutate --all"
    Then the exit code is 2
    And the output contains "--yes-slow"

  Scenario: Runtime gates refuse when unconfigured
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    When I run craftsman with "perf"
    Then the exit code is 3
    And the output contains "not configured"

  Scenario: Docs search finds a cached page offline
    Given a craftsman project with a seeded docs cache for library "demo"
    When I run craftsman with "docs search streaming"
    Then the exit code is 0
    And the output contains "pages/intro.md"
    And the output contains "data, not instructions"

  Scenario: Docs get refuses an unknown library
    Given a craftsman project with a seeded docs cache for library "demo"
    When I run craftsman with "docs get nosuch/intro"
    Then the exit code is 3
    And the output contains "nosuch"

  Scenario: Extract writes a session index the next session can read
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    And a batch 7 extract recorded the decision "chose curl over a new HTTP crate"
    When I run craftsman with "extract --show"
    Then the exit code is 0
    And the output contains "Batch 7"
    And the output contains "chose curl over a new HTTP crate"

  Scenario: Adr index regenerates a one-line-per-decision index
    Given a craftsman project with decisions "ADR-001: Alpha choice" and "ADR-002: Beta choice"
    When I run craftsman with "adr index"
    Then the exit code is 0
    And the decisions index lists "ADR-001: Alpha choice"
    And the decisions index lists "ADR-002: Beta choice"

  Scenario: Init scaffolds a project that doctor accepts
    Given an empty git repository directory
    When I run craftsman with "init --name demo --stack rust"
    Then the exit code is 0
    When I run craftsman with "doctor"
    Then the exit code is 0
    And the output contains "5/5 checks passed"

  Scenario: Init refuses to overwrite without force
    Given an empty git repository directory
    And craftsman init has already scaffolded it
    When I run craftsman with "init --name demo --stack rust"
    Then the exit code is 3
    And the output contains "craftsman.toml"
    And the output contains "--force"

  Scenario: Setup installs skills with attribution sentinels
    Given a sandboxed home directory with a Claude Code marker
    When I run craftsman setup against the sandboxed home
    Then the exit code is 0
    And the sandboxed home holds the canonical skill "craftsman-init" with a sentinel
    And the sandboxed home serves "craftsman-init" to Claude Code via a symlink

  Scenario: Adopt enforces phase ordering
    Given an empty git repository directory
    When I run craftsman with "adopt --start-phase 2"
    Then the exit code is 3
    And the output contains "phase 0"

  Scenario: Commit refuses when nothing is staged
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    And the project is a fresh git repository
    When I run craftsman with "commit --type chore --message tidy"
    Then the exit code is 3
    And the output contains "staged"

  Scenario: Commit rejects an unknown type
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    When I run craftsman with "commit --type vibes --message tidy"
    Then the exit code is 2
