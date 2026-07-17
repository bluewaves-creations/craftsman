# ADR-001: Gherkin → Swift Testing code-gen round trip (Spike S1)

**Status: Proven-with-constraints** — every claim below was executed on this machine (Apple Swift 6.3.3, swiftlang-6.3.3.1.3, macOS arm64, Testing Library version 1902) in `spikes/s1-swift-codegen/`. xcodebuild/xcresulttool variant: **deferred, not tested**.

## What was proven

`todo.feature` (2 scenarios + 1 Scenario Outline, 3 Examples rows) hand-translated to `Tests/SpecSpikeTests/TodoFeature.swift`: `@Suite("Feature: Todo management")`, one `@Test` per scenario with the scenario name as an SE-0451 raw-identifier function name, `.tags(.batch1, .todo)` traits, outline as `@Test(arguments: [(quantity: 0, reason: "zero"), …])` with the tuple destructured into two typed parameters. Steps live in `Steps.swift`; a minimal model in `Sources/SpecSpike/SpecSpike.swift`. `swift test` → all green, exit 0.

## Test identity (`swift test list`)

```
SpecSpikeTests.TodoManagementFeature/`Adding a todo shows it in the list`()
SpecSpikeTests.TodoManagementFeature/`Rejecting an invalid quantity keeps the cart unchanged`(quantity:reason:)
```

Backticks are part of the reported name; parameterized tests carry the argument-label signature.

## `--filter` rules (all verified by run counts)

- The filter is an **unanchored regex** matched against `Target.Suite/`name`()/File.swift:line:col` — proven: `'in the list`\(\)$'` matches 0, `'in the list`\(\)/TodoFeature\.swift:[0-9]+:[0-9]+$'` matches 1, `'^SpecSpikeTests'` matches 3. The trailing source location means **never `$`-anchor after the parens**.
- Backticks match literally; spaces need only shell quoting. `swift test --filter 'Adding a todo shows it in the list'` selected exactly 1 test.
- Unique-selection recipe for the generator: `--filter 'Target\.Suite/`<REGEX_ESCAPED_NAME>`\('` — proven to select exactly 1 even for a name containing `/`.
- Regex metacharacters in scenario names (`. ( ) [ ]` …) must be escaped; unescaped they still matched here but are correctness hazards.
- **Filter matching nothing exits 0** — confirmed: `--filter 'This matches no scenario at all'` → `EXIT=0`, "Test run with 0 tests". With `--xunit-output` it still writes a `tests="0"` XML. The adapter must count matches itself and map empty selection to craftsman exit 4.

## xUnit output

`swift test --parallel --xunit-output r.xml` (exit 0) writes `r.xml` (XCTest stub, `tests="0"`) **and sibling `r-swift-testing.xml`** — the real one. Also written *without* `--parallel` on this toolchain (7 testcases). Shape:

```xml
<testcase classname="SpecSpikeTests.TodoManagementFeature"
          name="`Adding a todo shows it in the list`()" time="0.000157333" />
```

Parameterized: **one** `<testcase name="`Rejecting …`(quantity:reason:)">` for all 3 rows — per-row results are NOT in the XML. Failures: `<failure message="Expectation failed: (list.todos → [&quot;Buy milk&quot;]).contains(…) (error): expected list to contain Buy oat milk" />`.

## JSONL event stream

`swift test --experimental-event-stream-output ev.jsonl --experimental-event-stream-version 0`. Version flag is **not validated**: 0, 1, 2 all accepted and all stamp `"version":0`; even `99` is accepted and stamps `"version":"99.0.0"` with identical record shapes. Generator must pin `0` and treat `version` in records as the contract.

Two record kinds. `kind:"test"` (discovery; `payload.kind` = `suite`|`function`):

```json
{"kind":"test","payload":{"_tags":[".todo",".batch1"],"displayName":"Adding a todo shows it in the list",
 "id":"SpecSpikeTests.TodoManagementFeature/`Adding a todo shows it in the list`()/TodoFeature.swift:18:6",
 "isParameterized":false,"kind":"function","name":"`Adding a todo shows it in the list`()", "sourceLocation":{…}},"version":0}
```

`kind:"event"` with `payload.kind` ∈ `runStarted, testStarted, testCaseStarted, testCaseEnded, testEnded, issueRecorded, runEnded`. Pass/fail is read from `testEnded` → `messages[].symbol` = `"pass"`/`"fail"` keyed by `testID` (same 3-part id as above; suites use `Target.Suite` only). Failures additionally emit:

```json
{"kind":"event","payload":{"kind":"issueRecorded","testID":"…/`Adding a todo shows it in the list`()/TodoFeature.swift:18:6",
 "issue":{"isKnown":false,"sourceLocation":{"fileID":"SpecSpikeTests/Steps.swift","line":30,…}},
 "messages":[{"symbol":"fail","text":"Expectation failed: (list.todos → [\"Buy milk\"]).contains(title → \"Buy oat milk\")"},
             {"symbol":"details","text":"expected list to contain Buy oat milk"}]},"version":0}
```

Parameterized cases: `testCaseStarted/Ended` carry `_testCase.displayName` (e.g. `"0, \"zero\""` — the argument values) and an `id` blob (`argumentIDs: […], discriminator: 0, isStable: true`); `testID` points at the parent function. So **per-row pass/fail exists only in the JSONL, not the XML** — the normalizer must consume JSONL as primary.

Deliberate failure: `swift test` → **exit 1** (verified without a pipeline; note `cmd | tail` masks the code).

## Name-mangling rules the Batch 5 generator must apply

Verified valid as raw identifiers AND uniquely filterable: unicode (`Café ferme à minuit — vérifié`), commas (`Adding 1,000 items, all at once`), quotes/parens/brackets, period, slash. Verified compile-breaking: **backslash** ("expected identifier"), **whitespace-only name**, and **duplicate scenario name in one suite** ("ambiguous use" from the @Test macro). Backtick and CR/LF cannot be written at all.

1. Reject (spec lint error) scenario names containing `` ` ``, `\`, or newline, and names that are empty/whitespace-only — do not silently rewrite.
2. Reject duplicate scenario names within a feature (compile error otherwise).
3. Otherwise emit the name verbatim in backticks — no other mangling needed.
4. For `--filter`, regex-escape the name and wrap: `Target\.Suite/`NAME`\(` — never anchor with `$`.
5. Treat 0 matched tests (exit still 0) as craftsman exit 4; parse JSONL v0 for per-scenario and per-Examples-row status; use `*-swift-testing.xml` only as a coarse fallback.

Commands that proved it: `swift test`, `swift test list`, `swift test --filter '…'`, `swift test --parallel --xunit-output r.xml`, `swift test --experimental-event-stream-output ev.jsonl --experimental-event-stream-version 0` — all in `spikes/s1-swift-codegen/`.
