# The spec harness

This directory is the cucumber-rs harness that executes the repo-root
`SPEC.md` against the compiled `craftsman` binary (`main.rs` is the
entry point; `craftsman verify` drives it with `CRAFTSMAN_JSON` set).
Step modules are grouped by surface; `fixtures.rs` holds the shared
fixture vocabulary; `probes.rs` holds the `@requires-*` capability
probes.

## Fixture lifecycle

Stable fixtures (compiled `target/`, `.venv`, `node_modules` survive
across runs) follow one lifecycle:

1. `fixtures::stable_dir("craftsman-spec-<name>-fixture")` — one
   directory **per scenario**; concurrently running scenarios must never
   share a fixture.
2. `fixtures::scrub(...)` — remove everything a previous run wrote
   (`.craftsman`, `.git`, `NOTES.md`, …) before building.
3. Build the tree.
4. When a repository is needed: `fixtures::git_init_commit_all` (or
   `fixtures::recommit_scaffold` for doctor-scaffolded trees, which also
   handles steps 2–3's git side and returns `HEAD`).

Priming runs (Given-steps that set state via the CLI) use `w.prime(...)`
— it asserts exit 0 loudly. The scenario's own When uses
`w.run_craftsman(...)`, whose exit code the Then judges.

## Traps (each cost a debugging session — keep them paid for)

- **Fixture idempotence**: anything a scenario writes into a stable
  fixture rides into the *next* run. A leftover `NOTES.md` committed
  into the fixture's initial commit means "a file is staged" later
  stages an identical file — nothing staged, exit 3. Scrub on entry,
  always (lesson of commit beb30bf).
- **Git identity lives in repo config, never `-c` flags**: per-command
  `-c user.name=…` only covers the test's own git calls. Commits the
  *CLI itself* makes inside the fixture resolve identity from config,
  and bare CI runners have no global identity — "empty ident name"
  (lesson of cbb5a08; reproduce locally with `HOME=<empty tempdir>`).
- **Cucumber-expression slashes are alternation**: in `#[given(expr =
  "…")]`, `src/a.py` parses as the alternation `src(a|.py)`. Escape
  literal slashes: `src\\/a.py`.
- **`--name` regex bypasses the programmatic filter** (cucumber 0.23
  `filter_run`): with a regex in the CLI opts, cucumber consults ONLY
  the regex and never calls the scenario filter. `main.rs` takes
  `re_filter` out of the opts and composes both in one closure — never
  hand cucumber the regex directly, or a name-filtered run can force a
  capability-gated scenario live (lesson of the `@requires-*` gate).
- **Capability probes must test the scenario's real precondition**: a
  runnable `xcodebuild` is not a buildable fixture — the xcode probe
  also requires Swift ≥ 6.2 because the generated tests use SE-0451 raw
  identifiers (lesson of 740aea5). When a gated scenario's toolchain
  needs grow, grow the probe with them.
- **Secret-shaped literals never enter this repo**: the repo's own
  gitleaks gate scans history. Fixture secrets are assembled at runtime
  from halves (see `security_steps::planted_secret`).
