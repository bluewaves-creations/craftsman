# Stack: Bash

Loaded when implementing shell scripts. The conventions file still binds.

Assumed floor: shellcheck (blocking) and shfmt as gates; every script opens with `set -euo pipefail`. Nothing below restates what they enforce. Shell is for thin glue — sequencing commands, moving files, wiring pipelines. The moment it becomes software, it graduates (see the checkpoint below).

## The graduation rule — hard checkpoint, not advice

Before growing any script, check these thresholds. Crossing **any one** means stop — do not write the next line of bash:

- ~100 lines of script
- arrays used as data structures (not just argv passthrough)
- nontrivial control flow (nested conditionals, state machines, retry loops)
- argument parsing beyond `$1`/`$2` (anything wanting `getopts` with several flags)

At the checkpoint: stop and propose — either the capability is a `craftsman` CLI feature request, or the script is rewritten in Python or Rust under the full gate stack. Never "just extend it this once"; growing scripts is the recorded failure mode, and each increment looks locally reasonable. Say which threshold tripped, propose the target, and wait for the human.

## Quoting discipline

Quote every expansion — `"$var"`, `"$(cmd)"`, `"$@"` — unless you can state why splitting is intended (and then comment it). Word splitting on an unquoted expansion is the highest-density latent failure in agent-written shell: it works in every test and detonates on the first path with a space.

```bash
# Bad: breaks on spaces, globs on '*', and $@ loses argument boundaries
rm -rf $BUILD_DIR/*
process $@

# Good
rm -rf "${BUILD_DIR:?}"/*      # :? also refuses to run with BUILD_DIR unset
process "$@"
```

Use `${var:?message}` on any variable whose emptiness would make a destructive command catastrophic.

## Cleanup with trap

Any script that creates temp files, starts background processes, or takes locks registers cleanup once, immediately after creating the resource — `trap` on `EXIT` covers success, failure, and Ctrl-C alike. Never rely on cleanup lines at the bottom of the script; `set -e` means execution may never reach them.

```bash
# Good: cleanup is guaranteed the instant the resource exists
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT
```

## Robust command output

- Never parse `ls` — filenames may contain spaces, newlines, anything. Iterate globs (`for f in "$dir"/*.log`) or use `find … -print0 | while IFS= read -r -d '' f`.
- Prefer `$(...)` — it nests and reads; backticks are banned by the formatter anyway.
- A pipeline's success means every stage succeeded (`pipefail` is on) — when a stage is *allowed* to fail (e.g. `grep` finding nothing), handle it explicitly: `grep pattern file || true`, with a comment if it isn't obvious.

```bash
# Bad: breaks on any whitespace in a filename
for f in $(ls "$dir"); do wc -l $f; done

# Good: the glob preserves filenames intact; nullglob avoids the no-match literal
shopt -s nullglob
for f in "$dir"/*.log; do wc -l "$f"; done
```

## Shape of a good script

- Functions for any block used twice or longer than a screen; `main "$@"` at the bottom so nothing executes at parse time.
- `local` for every function variable; UPPER_CASE only for exported/environment values.
- Fail loudly and early: check preconditions (commands exist via `command -v`, required args present) at the top, print errors to stderr (`echo "error: …" >&2`), exit nonzero.
- No interactive prompts in anything a gate or CI will run — take input from arguments and environment only.

When any of this starts feeling cramped — you want named options, structured output, unit tests — that feeling is the graduation checkpoint firing. Honor it.
