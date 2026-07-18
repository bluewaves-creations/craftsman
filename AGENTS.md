# Craftsman — AGENTS.md

This repo builds the Craftsman Dev system: the `craftsman` CLI (`cli/`, Rust), the six-skill family (`skills/`), and its documentation (`docs/`). It eats its own cooking: the conventions in `skills/craftsman-conventions.md` bind work in this repo.

## Commands (observed)

- Build: `cargo build` (in `cli/`; needs `PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"` on this machine)
- Test: `cargo test` · Lint: `cargo clippy --all-targets -- -D warnings` · Format: `cargo fmt`
- Skill validation: `uvx --from skills-ref agentskills validate skills/<skill-dir>`
- Fixture runners: `uv` for Python, `bun`/`bunx` for JS/TS (never npm/npx/node directly) — both on PATH on this machine
- Swift toolchain (spikes): Swift 6.3.3 via Xcode 26.x on this machine

## Hard constraints

- Exit-code contract: 0 pass · 1 verification failure · 2 usage · 3 orchestrator error · 4 empty selection. Every command: `--json` to stdout, human progress to stderr.
- No LLM calls anywhere in the CLI; no network in the verdict path; no telemetry.
- Single-writer: only the CLI writes `.craftsman/` state, baselines, trailers.
- Errors: `thiserror` enums in library modules; `anyhow` + `.with_context()` only in the command layer. No `unwrap` outside tests without a stated invariant comment.
- clippy pedantic+nursery are warnings locally, `-D warnings` in CI — fix, don't allow, unless the allow carries a reason.
- Fix and refactor never share a commit. Ledger trailer conventions per `skills/craftsman-conventions.md`.

## Authority chain

Design authority: `docs/design/2026-07-17-cli-surface-design.md` and `2026-07-17-skill-family-design.md`. Evidence: `docs/research/` (22 docs, see README index). Plan: `docs/plans/2026-07-17-cli-implementation-plan.md` — batched; revise at boundaries, don't silently drift from it.

## Documentation Sources

| Library / Surface | Source | Location | Pinned | Verify |
|---|---|---|---|---|
| clap | docsrs | https://docs.rs/clap/4 | 4.x | cargo check |
| gherkin (cucumber-rs) | docsrs | https://docs.rs/gherkin | 0.16.x | cargo check |
| cucumber-rs | llms.txt-less; book | https://cucumber-rs.github.io/cucumber/main/ | 0.23.x | cargo check |
| serde / toml | docsrs | https://docs.rs/toml | latest | cargo check |
| Swift Testing (spikes) | file | Apple docs via sosumi.ai URL-swap | Swift 6.3 | swift test |

Unlisted library → STOP and ask.
