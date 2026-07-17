# ADR-002: Normalized result schema v1 (Spike S2 verdict)

Status: accepted · Date: 2026-07-18 · Evidence: `spikes/s2-normalizer/` (samples, fixtures, prototype; `cargo test` 6/6 green). All facts below observed on this machine, not quoted from docs.

## Schema v1 (frozen)

```rust
pub enum Status { Passed, Skipped, Pending, Undefined, Ambiguous, Failed }

pub struct ScenarioResult {
    pub feature: String,
    pub scenario: String,
    pub status: Status,
    pub duration_ms: Option<u64>,
    pub failure: Option<String>,
}
```

Variant order doubles as severity; scenario status = `max` over step statuses (Passed < Skipped < Pending < Undefined < Ambiguous < Failed). Plain JUnit only expresses pass/failed/skipped, so every JUnit ingestion needs a per-runner dialect to recover the six-status vocabulary.

## Per-runner mapping (all observed against the same 3-scenario feature: pass / fail / one unimplemented step)

| Runner | Format ingested | Failed appears as | Undefined step appears as | Quirks |
|---|---|---|---|---|
| pytest-bdd 8.1.0 | `--cucumberjson=py.json` | step `result.status: "failed"` + `error_message` | **step absent** — undefined step and all later steps are omitted, scenario aggregates to PASSED | cucumber-json alone cannot detect UNDEFINED; durations are ns |
| pytest-bdd 8.1.0 | `--junitxml=py-junit.xml` | `<failure>` | `<failure>` whose message contains `StepDefinitionNotFoundError: Step definition is not found` → map to Undefined | testcase name is the mangled pytest id (`test_remove_an_item_from_the_list`), classname `tests.test_todo` — real scenario names need the cucumber-json or a de-mangling map |
| @cucumber/cucumber 13.1.1 | `--format message:msgs.ndjson` | `testStepFinished.testStepResult.status: "FAILED"` + `message` | `"UNDEFINED"` (subsequent step `"SKIPPED"`); a `suggestion` message carries the step snippet | richest source; duration is `{seconds, nanos}`; join chain gherkinDocument→pickle→testCase→testCaseStarted→testStepFinished |
| @cucumber/cucumber 13.1.1 | `--format json:ts.json` | step `"failed"` | step `result.status: "undefined"` explicitly | durations ns; statuses lowercase (Messages uses uppercase) |
| cucumber-rs 0.23.0 | JUnit (`output-junit`, `writer::JUnit`) | `<failure type="Step Panicked" message="Step panicked. Captured output: …"/>` | `<skipped/>` — only the `?  <step>` marker + `Step skipped:` in `<system-out>` distinguishes it from a skip → map to Undefined | names are `Scenario: <name>: <path>:<line>:<col>` / suite `Feature: <name>: <path>` — strip prefix + trailing path (breaks if a scenario name contains `": "`) |
| bats-core 1.13.0 | `--formatter junit` | `<failure type="failure">` with file/line text | n/a natively — craftsman's generated `skip "step not implemented: …"` shows as `<skipped>step not implemented: …</skipped>` → map to Undefined | no step concept; suite name = file name (`todo.bats`); testcase name = scenario name verbatim; hostname leaks into XML |

Nobody produced Pending or Ambiguous in these fixtures; both are reachable only via the JS family (`return 'pending'`, duplicate step defs) — vocabulary kept anyway (superset, costs nothing).

## Verified quirk list (command → observation)

- `uv run pytest tests/ -k "no_such_scenario_xyz"` → **exit 5** (empty match is an error-shaped exit, not 0/1). Normal failing run → exit 1.
- `npx cucumber-js --name "no_such_scenario_xyz"` → **exit 0**, prints `0 scenarios`. The adapter must count scenarios and convert 0 into craftsman exit 4 itself.
- `cargo test --test todo -- --name "Add" --tags "@wip"` → **exit 2**, `error: the argument '--name <regex>' cannot be used with '--tags <tagexpr>'` (clap `conflicts_with`, as the research doc claimed — now empirically confirmed).
- `cargo test --test todo -- --name "no_such_scenario_xyz"` → exit 0, nothing run (same empty-match trap as JS).
- cucumber-rs with `writer::JUnit` + `.run()` → **exit 0 even with a failing scenario**; the failure only exists inside the XML. A real adapter must use `run_and_exit`/inspect the result AND parse the XML, or treat the XML as the sole truth.
- `bats -f "no_such_scenario_xyz" todo.bats` → **exit 0**, prints `1..0` (TAP plan of zero). Same adapter-side empty-match handling needed.
- `bats --formatter junit todo.bats` → exit 1 on failure; JUnit goes to stdout (redirect it), and `skipped=` counts appear on the testsuite element.
- pytest-bdd cucumber-json truncation (the biggest surprise, not in the research doc): the undefined step vanishes from `steps`, so worst-of aggregation reports **PASSED**. Cross-check JUnit (StepDefinitionNotFoundError) or the exit code, or diff step counts against the .feature file.

## What Batch 2's `verify/normalize.rs` should copy vs redo

Copy nearly verbatim:
- `Status` + `ScenarioResult` + severity-ordered `max` aggregation.
- The three parser skeletons (`parse_messages_ndjson`, `parse_cucumber_json`, `parse_junit` with `JunitDialect`) and the dialect quirk rules — all fixture-proven.
- The fixtures themselves: move `spikes/s2-normalizer/fixtures/` into `cli/tests/fixtures/` and port the six assertions.

Redo properly:
- Error handling: prototype swallows malformed input (`filter_map`, `unwrap_or`); production wants `thiserror` parse errors per the repo conventions, and loud rejection of unknown status strings.
- cucumber-rs name de-mangling: replace the `rfind(": ")` string hack with something anchored on the known feature path, or (better) switch cucumber-rs ingestion to `output-json` cucumber-json where names are clean — evaluate in Batch 2.
- pytest-bdd adapter must merge both artifacts: cucumber-json for real scenario names/steps + JUnit for UNDEFINED detection (or de-mangle ids). Neither alone is sufficient — json lies about undefined, junit lies about names.
- Step-level results (`steps` field) were deliberately dropped from the spike struct; add when `verify --json` needs per-step reporting.
- Swift JSONL/xunit ingestion is not covered here — S1's fixtures feed a fourth dialect in Batch 5.
