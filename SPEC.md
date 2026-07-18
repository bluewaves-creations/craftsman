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
    And the scaffold's pinned gate tools are installed on this machine
    When I run craftsman with "init --name demo --stack rust"
    Then the exit code is 0
    When I run craftsman with "doctor"
    Then the exit code is 0
    And the output contains "6/6 checks passed"

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

  Scenario: Verify refuses a typescript project whose runner is not installed
    Given a typescript project that does not have the cucumber-js runner installed
    When I run craftsman with "verify"
    Then the exit code is 3
    And the output contains "@cucumber/cucumber"
    And the project lockfile is unchanged

  Scenario: Commit creates the first commit of a fresh repository
    Given a green craftsman project whose repository has no commits yet
    When I run craftsman commit for the staged tree
    Then the exit code is 0
    And the repository's only commit carries a Verified-by trailer

  Scenario: Init scaffolds a feature spec for the typescript stack
    Given an empty git repository directory
    When I run craftsman with "init --name web --stack typescript"
    Then the exit code is 0
    And the scaffold includes "features/web.feature"
    And the configured spec path ends with ".feature"

  Scenario: Doctor reports a pinned gate tool missing from the machine
    Given a craftsman project that pins a gate tool that does not exist on this machine
    When I run craftsman with "doctor"
    Then the exit code is 1
    And the output contains "gitleaks"

  Scenario: A baseline-mode refusal names the baseline command
    Given a craftsman project with a baseline-mode health gate, no recorded baseline, and an existing finding
    When I run craftsman with "health"
    Then the exit code is 1
    And the output contains "craftsman gate baseline health"

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
    And the output contains "max-function-lines"
    And no baseline was recorded

  Scenario: Import detects existing QA commands as conversion candidates
    Given a git repository with a package script named "qa"
    When I run craftsman with "import --name legacy --stack typescript"
    Then the exit code is 0
    And the output lists "qa" as a conversion candidate

  Scenario: A declared qa gate runs inside check-all
    Given a craftsman project declaring a qa gate whose command succeeds
    When I run craftsman with "check-all"
    Then the exit code is 0
    And the output contains "qa:smoke"

  Scenario: A red qa gate blocks commit
    Given a craftsman project declaring a qa gate whose command fails
    When I run craftsman commit for the staged tree
    Then the exit code is 1
    And no commit was created

  Scenario: A qa gate whose command is missing refuses loudly
    Given a craftsman project declaring a qa gate whose command does not exist
    When I run craftsman with "check-all"
    Then the exit code is 3
    And the output contains "craftsman-definitely-missing-xyz"

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

  # ————— Current behavior (recovered) — Batch 11 —————
  # Wired from SPEC.recover.md (approved by the human 2026-07-18).
  # Every scenario below was drafted verified-only: each cited a passing
  # test or an executed CLI observation before it was admitted here.

  Scenario: An unknown flag is a usage error
    Given any directory
    When I run craftsman with "spec status --no-such-flag"
    Then the exit code is 2

  Scenario: Init outside a git repository suggests git init
    Given an empty directory that is not a git repository
    When I run craftsman with "init --name demo --stack rust"
    Then the exit code is 3
    And the output contains "git init"

  Scenario: Init rejects an unknown stack
    Given an empty git repository directory
    When I run craftsman with "init --name demo --stack cobol"
    Then the exit code is 3
    And the output contains "cobol"
    And the output contains "known stacks"

  Scenario: Adopt status before any phase names phase 0 as next
    Given an empty git repository directory
    When I run craftsman with "adopt --status"
    Then the exit code is 0
    And the output contains "next phase is 0"

  Scenario: Adopt refuses to start the same phase twice
    Given an empty git repository directory
    And adoption phase 0 has been started
    When I run craftsman with "adopt --start-phase 0"
    Then the exit code is 3
    And the output contains "already"

  Scenario: Adopt phase 1 scaffolds a gates-off config and a baseline ADR
    Given an empty git repository directory
    And adoption phase 0 has been started and completed
    When I run craftsman with "adopt --start-phase 1"
    Then the exit code is 0
    And the file craftsman.toml exists
    And the file decisions/ADR-000-adoption-baseline.md exists

  Scenario: Adopt phase 1 leaves an existing config untouched
    Given a git repository with a hand-written craftsman.toml naming the project "keepme"
    And adoption phase 0 has been started and completed
    When I run craftsman with "adopt --start-phase 1"
    Then the exit code is 0
    And the config still names the project "keepme"

  Scenario: Learnings accumulate across batch extracts
    Given a craftsman project where batch 1 extracted the failed approach "first dead end"
    And batch 2 extracted the failed approach "second dead end"
    When I run craftsman with "extract --show"
    Then the exit code is 0
    And the learnings record contains both "first dead end" and "second dead end"

  Scenario: Extract show before any extract is a loud error
    Given a craftsman project where no extract has ever run
    When I run craftsman with "extract --show"
    Then the exit code is 3
    And the output contains "no session extract yet"

  Scenario: The decisions index never lists itself
    Given a craftsman project with the decision "ADR-001: Alpha choice" and a previously generated index
    When I run craftsman with "adr index"
    Then the exit code is 0
    And the regenerated index lists exactly 1 decision

  Scenario: A second setup run reports every skill up to date
    Given a sandboxed home directory where craftsman setup has already run
    When I run craftsman setup against the sandboxed home
    Then the exit code is 0
    And every canonical skill row reports "up-to-date"

  Scenario: Setup leaves a hand-modified skill tree in place
    Given a sandboxed home directory where a canonical skill tree holds an extra hand-written file
    When I run craftsman setup against the sandboxed home
    Then the exit code is 0
    And the modified skill row reports "left"
    And the hand-written file still exists

  Scenario: Setup with force replaces a modified skill tree and still lists it
    Given a sandboxed home directory where a canonical skill tree holds an extra hand-written file
    When I run craftsman setup with force against the sandboxed home
    Then the exit code is 0
    And the modified skill row reports "replaced"
    And the hand-written file no longer exists

  Scenario: Setup remove keeps modified trees and removes attributable ones
    Given a sandboxed home directory where one installed skill tree was hand-modified
    When I run craftsman setup remove against the sandboxed home
    Then the exit code is 0
    And the modified skill tree still exists
    And the unmodified skill trees and their agent links are removed

  Scenario: Spec status warns when the repository head moved after verify
    Given a craftsman project with a recorded green verify run
    And a commit has moved the repository head since that run
    When I run craftsman with "spec status"
    Then the exit code is 0
    And the output contains "may be stale"

  Scenario: Spec lint warns about regex-hostile scenario names without failing
    Given a craftsman project whose spec has a scenario named "Handles the (rare) case"
    When I run craftsman with "spec lint"
    Then the exit code is 0
    And the output contains a warning about the scenario name

  Scenario: Spec gen without a code-gen stack reports an empty selection
    Given a craftsman project configured with only the stack "rust"
    When I run craftsman with "spec gen"
    Then the exit code is 4
    And the output contains "swift"
    And the output contains "bash"

  Scenario: The a11y stub is written once and kept after hand edits
    Given a swift-stack craftsman project where the a11y stub was generated and then hand-edited
    When the a11y stub generation runs again
    Then the stub file reports "kept"
    And the hand edit is preserved

  Scenario: Plan lint rejects a scenario assigned to two batches
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    And its plan assigns the scenario "First behavior" to batch 1 and to batch 2
    When I run craftsman with "plan lint"
    Then the exit code is 1
    And the output contains "First behavior"

  Scenario: Plan lint warns about spec scenarios no batch covers
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    And its plan assigns batch 1 only the scenario "First behavior"
    When I run craftsman with "plan lint"
    Then the exit code is 0
    And the output warns about "Second behavior"

  Scenario: Docs sync of an undeclared library is a loud error
    Given a craftsman project with no docs source named "nosuchlib"
    When I run craftsman with "docs sync nosuchlib"
    Then the exit code is 3
    And the output contains "nosuchlib"

  Scenario: Docs sync with no sources declared is an empty selection
    Given a craftsman project with no docs sources declared
    When I run craftsman with "docs sync"
    Then the exit code is 4
    And the output contains "docs add"

  Scenario: A local file source syncs and searches without network
    Given a craftsman project with a file docs source pointing at a local markdown directory
    When the source is synced and then searched for its content
    Then both commands exit 0
    And the search names the local page

  Scenario: Docs search with zero hits still exits cleanly
    Given a craftsman project with a synced docs cache
    When I run craftsman with "docs search zzzznotthere"
    Then the exit code is 0
    And the output contains "0 hit(s)"

  Scenario: Docs get on an unknown page lists the pages that exist
    Given a craftsman project with a seeded docs cache for library "demo" holding pages intro.md and faq.md
    When I run craftsman with "docs get demo/ghost"
    Then the exit code is 3
    And the output names the pages that do exist

  Scenario: Syncing a newer version replaces the older cached copy
    Given a docs cache holding library "demo" at version 1.0.0
    When version 2.0.0 of "demo" is synced
    Then the cache holds version 2.0.0
    And the 1.0.0 copy is gone

  @requires-network
  Scenario: Syncing an llms-txt source caches its markdown pages
    Given a craftsman project with an llms-txt docs source for a live library
    When I run craftsman docs sync for that library
    Then the exit code is 0
    And the cached pages are markdown files searchable offline

  Scenario: Commit with green gates records a Verified-by trailer
    Given a craftsman project whose gates are all green
    And a file is staged
    When I run craftsman commit with type "feat" and message "observe ledger"
    Then the exit code is 0
    And the new commit message carries a Verified-by trailer naming the gates that ran

  Scenario: Commit with a red gate refuses and commits nothing
    Given a craftsman project whose verify gate is red
    And a file is staged
    When I run craftsman commit with type "fix" and message "should be refused"
    Then the exit code is 1
    And the output contains "nothing committed"
    And the repository head is unchanged

  Scenario: Commit rejects a hand-written Verified-by trailer
    Given a craftsman project whose spec has scenarios "First behavior" and "Second behavior"
    And a file is staged
    When I run craftsman commit with a learned line containing "Verified-by: forged"
    Then the exit code is 3
    And the output contains "written by the CLI"

  Scenario: A new finding blocks a baseline-mode gate
    Given a gate in baseline mode with 2 recorded findings
    And the code now produces 1 of the recorded findings plus 1 fresh finding
    When the gate runs
    Then the exit code is 1
    And only the fresh finding is reported as blocking

  Scenario: Fixing a baselined finding shrinks the baseline permanently
    Given a gate in baseline mode with 2 recorded findings
    And the code now produces only 1 of them
    When the gate runs in full
    Then the baseline is ratcheted down to 1 entry
    And the ratchet is recorded with a timestamp

  Scenario: A changed-scope gate run never ratchets the baseline
    Given a gate in baseline mode with 2 recorded findings
    And a changed-scope run produces no findings
    When the gate runs with changed scope
    Then the exit code is 0
    And the baseline still holds 2 entries

  Scenario: Gate strict flips the config once baseline debt is zero
    Given a craftsman project whose arch gate is in baseline mode with zero baseline debt
    When I run craftsman with "gate strict arch"
    Then the exit code is 0
    And the config line for the arch gate now reads strict
    And no other config line changed

  Scenario: Gate status lists every gate with its mode and debt
    Given a craftsman project whose config sets verify to strict and lint to baseline
    When I run craftsman with "gate status"
    Then the exit code is 0
    And the output lists 9 gates each with a mode and a baseline count

  Scenario: A health allow directive with a reason suppresses one finding
    Given a source file whose over-long function is preceded by a craftsman-health allow directive carrying a reason
    When I run craftsman with "health"
    Then the exit code is 0
    And no finding is reported for that function

  Scenario: A health allow directive without a reason is itself a finding
    Given a source file whose over-long function is preceded by a craftsman-health allow directive with no reason
    When I run craftsman with "health"
    Then the exit code is 1
    And a finding reports the reasonless allow directive
    And the over-long function is still reported

  Scenario: Duplicated blocks across files merge into one finding
    Given two source files sharing an identical 12-line block
    When I run craftsman with "health"
    Then the exit code is 1
    And one duplication finding names both locations

  Scenario: Security findings never reveal the detected secret
    Given a repository whose history contains a committed API key
    When I run craftsman with "security"
    Then the exit code is 1
    And the finding names the file and rule
    And the output does not contain the secret value

  Scenario: Doctor observes a red verdict before the green round trip
    Given a craftsman project whose tools are installed
    When I run craftsman with "doctor"
    Then the exit code is 0
    And the round-trip check reports both a red and a green observation

  Scenario: Verify scenario filter runs only the named scenario
    Given a craftsman project with passing scenarios "First behavior" and "Second behavior"
    When I run craftsman verify for the scenario "First behavior"
    Then the exit code is 0
    And exactly one scenario result is reported, named "First behavior"

  Scenario: An undefined step carries the runner evidence
    Given a craftsman project whose spec has a scenario with an unimplemented step
    When I run craftsman with "verify"
    Then the exit code is 1
    And the undefined scenario result carries the runner's missing-step detail

  Scenario: A full verify run records the files each scenario covers
    Given a python craftsman project with no impact map
    When I run craftsman with "verify"
    Then an impact map exists mapping each covered scenario to the files it executed

  Scenario: Impact runs only scenarios whose covered files changed
    Given a craftsman project whose impact map covers "First behavior" with src/a.py and "Second behavior" with src/b.py
    And the diff since the last commit touches only src/a.py
    When I run craftsman verify with impact selection
    Then only the scenario "First behavior" runs

  Scenario: Scenarios unknown to the impact map always run
    Given a craftsman project whose impact map covers "First behavior" but not "New behavior"
    And the diff since the last commit touches no covered file
    When I run craftsman verify with impact selection
    Then the scenario "New behavior" runs

  @requires-swift
  Scenario: Generated swift scenarios go red when a step assertion fails
    Given a swift-stack craftsman project with generated scenarios whose step asserts a counter holds 2
    And the step implementation makes the counter hold 3
    When I run craftsman with "verify"
    Then the scenario is reported failed, not undefined
    And the failure detail names the actual counter value

  @requires-xcode
  Scenario: Apple UI test scenarios report pass, undefined and fail distinctly
    Given a swift-apple craftsman project with one passing, one unimplemented, and one failing scenario
    When I run craftsman with "verify"
    Then the exit code is 1
    And the three scenarios are reported as passed, undefined, and failed respectively

  Scenario: Survived mutants below the score threshold block the mutate gate
    Given a craftsman project whose diff touches code with weak tests
    And the mutate minimum score is 100
    When I run craftsman with "mutate"
    Then the exit code is 1
    And the output reports the score against the threshold
    And survived mutants are reported as findings

  Scenario: A clean tree reports nothing to mutate as a pass
    Given a craftsman project with no uncommitted changes
    When I run craftsman with "mutate"
    Then the exit code is 0
    And the output contains "nothing to mutate"

  Scenario: Mutate refuses stacks without a consensus tool
    Given a craftsman project configured with only the stack "swift"
    When I run craftsman with "mutate"
    Then the exit code is 3
    And the output contains "not supported for stack swift"

  @requires-chromium
  Scenario: A visual regression blocks with a finding naming the failing spec
    Given a craftsman project with a configured visual gate whose page drifted from its committed baseline
    When I run craftsman with "visual"
    Then the exit code is 1
    And a failed-spec finding names the failing spec file

  @requires-chromium
  Scenario: An accessibility failure blocks the a11y gate
    Given a craftsman project with a configured a11y gate whose page carries a seeded accessibility issue
    When I run craftsman with "a11y"
    Then the exit code is 1
    And a failed-spec finding is reported with its line

  # ————— Batch 12 promises (delta approved by the human 2026-07-18) —————

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
