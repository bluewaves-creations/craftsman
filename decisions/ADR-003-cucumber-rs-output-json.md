# ADR-003: cucumber-rs verify adapter ingests `output-json`, not JUnit

Status: accepted · Date: 2026-07-18 · Evidence: probe run of the S2 rust-todo
sample rebuilt with cucumber 0.23 `output-json` (fixture committed as
`cli/tests/fixtures/rust.json`); all facts observed on this machine.

## Context

ADR-002 froze schema v1 and flagged the cucumber-rs JUnit ingestion as the
weak spot: testcase names arrive as `Scenario: <name>: <path>:<line>:<col>`,
and the `rfind(": ")` de-mangling breaks for any scenario name containing
`": "`. It asked Batch 2 to evaluate switching to `output-json` cucumber-json.

## Decision

`craftsman verify` for the rust stack ingests **cucumber-json produced by
cucumber-rs's `output-json` feature** (`writer::Json`), under a dedicated
`CucumberJsonDialect::CucumberRs`. The JUnit dialect stays in the normalizer
as a fixture-proven fallback only.

## Observed facts (cucumber 0.23.0, this machine)

- Names are clean: `elements[].name` is the scenario name verbatim,
  `[].name` (feature) likewise — no path suffix, no keyword prefix, no
  de-mangling hazard.
- Failures carry a structured `error_message` on the failing step
  (`"Step panicked. Captured output: …"`), plus `line` per step.
- An unmatched (undefined) step appears as `result.status: "skipped"` and
  all later steps are **omitted** from `steps`. cucumber-rs has no other
  producer of step-level skips (tag-filtered scenarios are absent from the
  output entirely), so the dialect maps step `skipped` → `Undefined`.
  Unlike the pytest-bdd trap (ADR-002), the marker step itself is present,
  so no cross-artifact merge is needed.
- A mid-scenario failure also omits later steps; worst-of aggregation is
  unaffected (`failed` is already the max).
- `--name` filter matching nothing: exit 0 and a well-formed empty `[]`
  JSON file — the adapter counts scenarios itself and maps 0 to craftsman
  exit 4 (same trap as JUnit, now with a parseable artifact).
- `-- --name '^…$'` anchors work; an anchored alternation of regex-escaped
  names selects exactly the requested scenarios.
- Exit code stays 0 even with failing scenarios when a file writer is used
  (re-confirming ADR-002): the JSON artifact is the sole truth.

## Consequences

- The harness convention for the rust stack: the project's cucumber-rs test
  target writes cucumber-json to the path given in the `CRAFTSMAN_JSON`
  environment variable (this repo's `cli/tests/spec.rs` implements it).
- `JunitDialect::CucumberRs` is retained but no longer on the verify path;
  its `": "` hazard is documented at the parsing site.
