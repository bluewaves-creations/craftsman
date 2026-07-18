# Craftsman — AGENTS.md

This repo builds the Craftsman Dev system: the `craftsman` CLI (`cli/`, Rust), the six-skill family (`skills/`), and its documentation (`docs/`). It eats its own cooking: the conventions in `skills/craftsman-conventions.md` bind work in this repo.

## Commands (observed)

- Build: `cargo build` (in `cli/`; needs `PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"` on this machine)
- Test: `cargo test` · Lint: `cargo clippy --all-targets -- -D warnings` · Format: `cargo fmt`
- Install: `sh install.sh` (release binary or cargo, then `craftsman setup`) · Release config: `dist` (cargo-dist 0.32.0, Homebrew)
- Install (team, from the release — writes the update receipt): `curl -LsSf https://github.com/bluewaves-creations/craftsman/releases/latest/download/craftsman-installer.sh | sh` — thereafter `craftsman update` self-updates
- Skill validation: `uvx --from skills-ref agentskills validate skills/<skill-dir>`
- Fixture runners: `uv` for Python, `bun`/`bunx` for JS/TS (never npm/npx/node directly) — both on PATH on this machine
- Swift toolchains: Xcode 26.6 (selected, /Applications/Xcode.app) and Xcode 27.0 (/Applications/Xcode-beta.app — the team's target; select via DEVELOPER_DIR=/Applications/Xcode-beta.app/Contents/Developer); xcodebuild round trip proven under both (2026-07-18)

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
| clap | docsrs-json | https://docs.rs/crate/clap | 4.x (Cargo.lock) | cargo check |
| gherkin (cucumber-rs) | docsrs-json | https://docs.rs/crate/gherkin | 0.16.x (Cargo.lock) | cargo check |
| cucumber-book (cucumber-rs book) | llms-txt | https://raw.githubusercontent.com/cucumber-rs/cucumber/main/book/src/SUMMARY.md | 0.23.x | cargo check |
| serde / toml | docsrs-json | https://docs.rs/crate/toml | latest | cargo check |
| Swift Testing (spikes) | file | Apple docs via sosumi.ai URL-swap | Swift 6.3 | swift test |
| specspike (docc dogfood) | docc | spikes/s1-swift-codegen | latest | swift build |
| pydantic (objects-inv dogfood) | objects-inv | https://docs.pydantic.dev/latest/objects.inv | latest | — |
| zod (dts dogfood) | dts | cli/tests/fixtures/ts-todo node_modules | 4.3.5 (bun.lock) | bun test |
| axoupdater | docsrs-json | https://docs.rs/crate/axoupdater | 0.10.0 | cargo check |

Sources are synced into `.craftsman/docs/` via `craftsman docs add`/`sync`; query offline with `craftsman docs search <query> [--lib <name>]`. Unlisted library → STOP and ask.
