//! File templates written by `craftsman init` and `craftsman adopt`.
//!
//! Skeletons only: section headings and mechanical structure. Content is
//! the human's (via the craftsman-init interview) — a missing section
//! beats an invented one, so nothing here fabricates project prose.

/// The AGENTS.md skeleton: headings + the Documentation Sources table
/// header + the closing STOP line. `__NAME__` is substituted.
pub const AGENTS_MD: &str = "\
# __NAME__ — AGENTS.md

<!-- budget: 100 lines of rules — overflow is either a gate rule (move it \
to craftsman.toml) or not load-bearing (cut it) -->

## Purpose

<!-- 2–3 sentences: what this is and who it serves. Human-attested via the
     craftsman-init interview — never generated. -->

## Commands (observed)

<!-- Only commands that actually ran successfully on this machine.
     Build: … · Test: … · Run: … -->

## Hard constraints

<!-- The non-negotiables. For each: can a gate enforce it? If yes, it
     becomes a craftsman.toml rule and this file just points there. -->

## Taste

<!-- What good looks like — one concrete code example per convention. -->

## Documentation Sources

| Library / Surface | Source | Location | Pinned | Verify |
|---|---|---|---|---|

Unlisted library → STOP and ask.
";

/// The walking-skeleton SPEC.md. `__NAME__` is substituted. Steps stay
/// abstract: the first implementation session makes them concrete with
/// the human (the spec is human-owned even at line one).
pub const SPEC_MD: &str = "\
Feature: __NAME__ walking skeleton

  The first scenario exists to prove the loop closes: observed red,
  implemented, observed green through `craftsman verify`. Human-owned —
  only the human changes acceptance criteria.

  Scenario: The walking skeleton responds
    Given the project is set up
    When the entry point runs
    Then it reports success
";

/// `.claude/settings.json` — hook wiring for Claude Code.
///
/// Shape verified against Claude Code's settings format (top-level
/// `hooks` map of event → matcher groups → hook commands; command hooks
/// read a JSON payload on stdin; exit 2 blocks and feeds stderr back).
/// `PreToolUse` guards `git commit` behind the gates; `Stop` refuses to stop
/// while the gates are red. Both call the committed enforcement:
/// `craftsman check-all --changed`.
pub const CLAUDE_SETTINGS_JSON: &str = r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "sh -c 'payload=$(cat); case \"$payload\" in *\"git commit\"*) cd \"$CLAUDE_PROJECT_DIR\" && craftsman check-all --changed >&2 || { echo \"craftsman gates are red — use craftsman commit after they pass\" >&2; exit 2; };; esac'",
            "timeout": 600
          }
        ]
      }
    ],
    "Stop": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "sh -c 'cd \"$CLAUDE_PROJECT_DIR\" && craftsman check-all --changed >&2 || { echo \"craftsman gates are red — fix before stopping\" >&2; exit 2; }'",
            "timeout": 600
          }
        ]
      }
    ]
  }
}
"#;

/// `.cursor/craftsman-hooks.json.template` — deliberately a `.template`,
/// not a live config: Cursor's hooks JSON shape could not be verified from
/// an offline docs source when this was authored, and craftsman never
/// invents a schema silently. The `_note` says exactly that.
pub const CURSOR_HOOKS_TEMPLATE: &str = r#"{
  "_note": "TEMPLATE, not live config. Cursor's hooks JSON shape was not verifiable from an offline docs source when craftsman generated this file, so it ships inert. Intent to port: run `craftsman check-all --changed` before any git commit and when the agent finishes, blocking on non-zero exit. Verify the current schema at https://cursor.com/docs, adapt, and rename/move this file accordingly. Until then, enforcement holds structurally: `craftsman commit` refuses while gates are red, and CI runs `craftsman check-all`.",
  "hooks": {
    "before-commit": "craftsman check-all --changed",
    "on-stop": "craftsman check-all --changed"
  }
}
"#;

/// The pointer file written where a CLAUDE.md symlink cannot be created.
pub const CLAUDE_POINTER_MD: &str = "\
Read AGENTS.md — the single source of project instructions.

(This file exists only because this harness looks for CLAUDE.md; a symlink
could not be created on this filesystem. Do not add content here.)
";

/// `.gitignore` lines init guarantees are present (merged, never
/// overwritten). Per the design doc, `.craftsman/` is gitignored except
/// `baselines/` and `adoption.toml` — the committed ratchet memory.
pub const GITIGNORE_LINES: &[&str] = &[
    ".craftsman/cache/",
    ".craftsman/session/",
    ".craftsman/docs/",
];

/// craftsman.toml for `init`: verify + lint strict, security baseline —
/// the greenfield defaults for every stack set. `__NAME__`, `__STACKS__`,
/// `__VERSION__` are substituted.
pub const INIT_CONFIG_TOML: &str = "\
# craftsman.toml — the committed contract between human, agent, CLI, and CI.
# Written by `craftsman init`; the craftsman-init skill interview refines it.

[project]
name = \"__NAME__\"
stacks = [__STACKS__]
spec = \"__SPEC__\"
cli-version = \"__VERSION__\"

[gates]
# Greenfield: strict from birth; there is no debt to baseline.
verify = \"strict\"
lint = \"strict\"
security = \"baseline\"   # scanners see the world's CVEs, not your code alone

[gates.tools]
gitleaks = \"8.24.0\"
semgrep = \"1.146.0\"
osv-scanner = \"2.4.0\"

[budgets]
tokens.agents-md-lines = 100
";

/// craftsman.toml for `adopt --start-phase 1`: process only, gates off.
/// `__NAME__` and `__VERSION__` are substituted.
pub const ADOPT_CONFIG_TOML: &str = "\
# craftsman.toml — written by `craftsman adopt --start-phase 1` (ledger phase).
# Brownfield ordering: ledger before gates, gates before specs, specs before
# change. Every gate except verify starts off; Phase 2 records baselines and
# flips them to \"baseline\" as the debt is made visible.

[project]
name = \"__NAME__\"
stacks = []   # fill in: swift-apple | swift | python | typescript | rust | bash
cli-version = \"__VERSION__\"

[gates]
verify = \"strict\"   # strict from birth — the spec is empty until Phase 3
";

/// decisions/ADR-000 for `adopt --start-phase 1`. `__DATE__` and `__HEAD__`
/// are substituted; the Consequences section is the human's.
pub const ADR_000: &str = "\
# ADR-000: State of the system at adoption

**Status: accepted** · Date: __DATE__

## Context

Craftsman adoption began at commit `__HEAD__`. This record pins the baseline:
what the system was when the ledger started, so every later decision has a
fixed point to diff against.

## Decision

Adopt the Craftsman methodology via the five-phase brownfield protocol
(observe → ledger → baseline gates → recover truth → steady state), phase
state tracked in `.craftsman/adoption.toml`.

## Consequences

<!-- Filled from Phase 0's survey — verified claims only; inferred material
     stays in docs/craftsman/adoption-survey.md with its labels. -->
";
