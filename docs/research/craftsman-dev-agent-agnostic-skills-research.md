# Agent-Agnostic Skill Architecture: The House Pattern and the 2026 Landscape

> How Craftsman Dev's skill set stays portable across Claude Code, Codex, Cursor, Gemini CLI and whatever comes next — extracted from two shipped in-house projects (Fusion, Shaping Rooms) and validated against the Agent Skills standard, the harness matrix, and the skill supply-chain incidents of early 2026.

---

## The Question

Craftsman Dev is a methodology first, a skill set second, a CLI third. For the skill set to matter it must run on any serious harness — the same requirement Fusion and Shaping Rooms already met. Two sub-questions: (1) what is the proven house pattern in those two repos, and what should be repeated vs. improved? (2) What does the mid-2026 ecosystem — the agentskills.io standard, harness support, distribution channels, supply-chain reality — demand or forbid?

## The House Pattern: Two Shipped Data Points

### Fusion (open source, MIT, PyPI `fusion-cli`)

Repo shape at `~/Developer/fusion`: `SPEC.md` at root ("the convention is the actual product"), `skills/` (four `fusion-*` skills + a family README), `cli/` (Python, uv, hatch), `install.sh` + `install.ps1`, `docs/{plans,specs,acceptance,dogfood}`, `examples/crazy-ones` (a fully conformant example bucket). The README positions three layers explicitly: **convention** (SPEC.md), **CLI** ("the notary"), **skill family** ("the judgment") — "for any agent that reads Agent Skills — Claude Code, Pi, Goose, and whatever comes next." This is the Triad Pattern under another name, shipped twice.

The four-point contract every Fusion skill obeys (from `skills/README.md`):

1. **Self-contained** — agentskills.io standard, PEP 723 scripts via `uv run`, "no harness-specific anything."
2. **Judgment proposes, code records** — every ledger/register write goes through a single deterministic writer (`fusion log`, `fusion index`); skills never hand-edit generated files.
3. **The convention travels** — each skill carries a byte-identical `references/fusion-conventions.md` (161 lines), so a skill installed alone still knows the house rules.
4. **One skill, one accountability.**

The SKILL.md house style is consistent across all four skills (53–148 lines each):

```yaml
---
name: fusion-librarian
description: "The Fusion librarian — the accountable owner of a bucket's
  order. One entry, eight gears: query (…the default), create, tag, …
  Use for 'find', 'search', 'where is', 'create a document', ….
  For files arriving from outside use fusion-intake; for deliverables use
  fusion-analyst. Applies only inside a Fusion bucket — a directory tree
  with BUCKET.md and LEDGER.md at its root; if there is no such bucket in
  play, offer to create one with `fusion new` instead of improvising."
license: MIT
compatibility: "Requires the fusion CLI on PATH."
---
```

Five description moves, in order: **identity** ("the accountable owner"), **gear enumeration** with an explicit safe default, **literal trigger phrases** in quotes, **cross-routing** to sibling skills ("for X use Y"), **applicability gate** with a graceful exit. The body then follows a fixed skeleton: identity paragraph → "read `references/<conventions>.md` once per session" → a routing table (`Signal | Gear | Load references/<gear>.md`) → destructive-gear rule ("never reached by near-miss inference — stop and confirm by name") → a ledger-verb table → a closing `## Never` list. Progressive disclosure is real: SKILL.md stays under 150 lines; per-gear references run 12–107 lines and load only when that gear fires.

Installation is a **thin bootstrap + tested brain**. `install.sh` is ~70 lines of POSIX sh: ensure uv, `uv tool install --force --refresh fusion-cli`, then `exec fusion setup`. Every mechanical decision lives in `cli/src/fusion/setup.py` — Python, unit-tested, idempotent, identical on macOS/Linux/Windows. Setup installs the canonical payload into `~/.agents/skills/`, then consults a per-agent table:

| Agent | Detection marker | Mode |
|---|---|---|
| Claude Code | `~/.claude` | **link** — symlink each skill into `~/.claude/skills/` (copy + sentinel fallback) |
| Codex | `~/.codex` | standard — reads `~/.agents/skills` natively; sweep legacy links |
| Pi | `~/.pi` | standard (legacy `~/.pi/agent/skills` swept) |
| Cursor | `~/.cursor` | standard |
| Gemini CLI | `~/.gemini` | standard |
| opencode | `~/.config/opencode` | standard |
| Goose | `~/.config/goose` | standard |

The table is data, not code — one row per agent, declaring a detection marker, a mode, and a docs URL:

```python
AGENTS = [
    {"name": "Claude Code", "marker": ".claude",
     "skills_subdir": ".claude/skills", "mode": "link"},
    {"name": "Codex", "marker": ".codex",
     "skills_subdir": ".agents/skills", "mode": "standard",
     "legacy_subdir": ".codex/skills"},   # swept: double-loading fix
    # … Pi, Cursor, Gemini CLI, opencode, Goose — all "standard"
]
```

Three details worth copying verbatim:

- **Never-destroy semantics with attribution.** Setup only removes what it can prove it created: symlinks resolving into the canonical dir, copies carrying a `.fusion-setup` sentinel, or trees matching a sha256 digest. Everything foreign is reported and left unless `--force`. The `remove` path mirrors the same proofs.
- **Legacy sweep.** When an agent gains native `~/.agents/skills` support, old per-agent links would double-load every skill; setup sweeps its own leftovers (attribution-checked) and reports the rest.
- **Payload bundling.** A hatch build hook stages `skills/fusion-*` into the wheel as `fusion/_skills` at every build, so the PyPI package carries the skills and `fusion update` refreshes CLI + skills in one verb — the repo's `skills/` directory stays the single source of truth, and uv verifies PyPI hashes on every install.

### Shaping Rooms (private sibling, July 2026 — the later iteration)

Same skeleton (`skills/` + `cli/` + docs), five deliberate evolutions:

1. **Vendoring with a lockfile.** The universal `doc-converter` skill has one canonical home (the public `skillsboutique-skills` library); Shaping Rooms carries a full copy pinned by `vendor.lock.json`:

   ```json
   {
     "skill": "doc-converter",
     "canonical": "skillsboutique-skills",
     "path": "skills/doc-converter",
     "commit": "d39bf090f96330f38fbb0c8ec824e0b44effbd6a",
     "tree": "49b7768c738e1735412b965a6d9b65854e5878cf"
   }
   ```

   `tree` is the **git tree hash** of the skill directory — content-addressed, so identical content hashes identically in any repo and drift is detectable offline, no network. `scripts/vendor-doc-converter.sh` refreshes via `git archive` (committed tree only; refuses a dirty canonical); `scripts/check-vendor.sh` fails if the copy was edited in place. "One source of truth, N convenient copies, drift mechanically impossible to miss."
2. **The CLI owns the canonical template payload** (resolved 2026-07-12): scaffold/upgrade skills deleted their bundled templates and delegate mechanics to `shaping new` / `shaping upgrade`, keeping only interview and judgment. Skills shrink; the CLI grows.
3. **Agent-agnostic workspaces**: the tracked guidance file is `AGENTS.md`; "the CLAUDE.md symlink is the single tolerated harness artifact" (with a pointer-file fallback where symlinks fail).
4. **`allowed-tools` frontmatter** appears, plus a family-level "umbrella" portability contract that individual skills cite when they claim an exception ("quality-over-portability, umbrella §3") — exceptions are named and numbered, not silent.
5. **No default gear for destructive-only skills** (shapr-workspace: ambiguous → ask), refining Fusion's rule that only read-only gears may be defaults. And **mutual-exclusion context gates** across families: every description says which workspace type it applies to, what to use in the other, and to stop if neither.

Internal distribution collapsed to three lines: `uv tool install ./cli`, `rsync -a skills/ ~/.agents/skills/`, symlink `~/.claude/skills` if a harness wants it. The lesson: the installer's sophistication should be proportional to the audience, not to the author's pride.

## The Agent Skills Standard (agentskills.io, mid-2026)

Originally Anthropic's format, now an open standard developed at github.com/agentskills/agentskills (23k stars; Apache-2.0 code, CC-BY-4.0 docs; `skills-ref validate` reference validator). No published semver — it is a living spec. The essentials:

- A skill = a directory whose name **must match** frontmatter `name` (1–64 chars, `^[a-z0-9]+(-[a-z0-9]+)*$`), containing `SKILL.md` plus optional `scripts/`, `references/`, `assets/`.
- Frontmatter: `name` and `description` (1–1024 chars) required; `license`, `compatibility` (≤500 chars), `metadata` (string map for client-specific properties) optional; `allowed-tools` optional and **experimental** — support varies.
- Three-tier progressive disclosure: metadata (~50–100 tokens/skill, always loaded) → SKILL.md body on activation (keep <500 lines / <5k tokens) → resources on demand, relative paths one level deep.
- Discovery is deliberately unspecified, but the implementer guide blesses **`.agents/skills/` (project) and `~/.agents/skills/` (user)** as "a widely-adopted convention for cross-client skill sharing," with project overriding user on collisions.

Fusion's skills were already spec-conformant; the standard has since converged on exactly the conventions the house pattern uses. No migration cost.

## The Harness Matrix (mid-2026)

| Harness | Native SKILL.md | Reads `.agents/skills` | Notes |
|---|---|---|---|
| Claude Code | ✓ | **✗** — only `.claude/skills` (project + user + nested monorepo), plugins | Commands merged into skills; extensions: `context: fork`, `disable-model-invocation`, `arguments`, `$1` substitution |
| OpenAI Codex | ✓ | ✓ (repo + `~` + `/etc/codex/skills`) | `$skill-name` invocation; ~2% context / 8,000-char catalog budget; optional `agents/openai.yaml` |
| Cursor | ✓ | ✓ (+ legacy `.claude/skills`, `.codex/skills`) | `/migrate-to-skills` converts old `.mdc` rules; extra `paths` glob frontmatter |
| Gemini CLI | ✓ | ✓ (alias takes precedence) | `activate_skill` behind a **user consent prompt**; `gemini skills install` |
| opencode | ✓ | ✓ (+ `.claude/skills`, `.opencode/skills`) | Spec-strict name validation; per-skill permission patterns |
| Goose | ✓ (v1.25.0+) | ✓ | Built-in "Summon" extension; Block runs github.com/block/agent-skills |
| Amp | ✓ | ✓ (+ `.claude/skills`) | Skills can bundle MCP servers via `mcp.json` |
| Zed | ✓ (v1.4.2) | unverified | Replaced Rules Library; global AGENTS.md; scan paths not confirmed |
| Windsurf | ✓ | ✗ — `.windsurf/skills/`, `~/.codeium/windsurf/skills/` | Docs now redirect to devin.ai (Cognition acquisition); paths may churn |

The picture inverted since 2025: the least common denominator is no longer AGENTS.md prose — it is **the spec-strict skill directory installed at `.agents/skills/` + `~/.agents/skills/`**, natively scanned by everything except Claude Code (symlink adapter, explicitly supported), Windsurf (copy adapter), and Zed (unverified). AGENTS.md (agents.md, now stewarded by the Agentic AI Foundation under the Linux Foundation, 60k+ repos) remains the always-on strategy layer — which is exactly the Instructions leg of the Triad, not a fallback for skills. Harness-specific frontmatter (Claude's `context`/`arguments`, Cursor's `paths`) is ignored elsewhere under the spec's lenient-parsing model, so portable skills stick to spec fields.

## Description Craft: What Makes Skills Fire

Anthropic's best-practices doc (platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices) is the prescriptive source: third person always (point-of-view inconsistency measurably hurts discovery), what + when + concrete trigger nouns, ≤1,024 chars, no XML. Field evidence adds three things:

- **Models undertrigger.** Descriptions should be "pushy" and enumerate literal user phrasings. Community consensus: "if your skill doesn't trigger, it is almost never the instructions — it is the description."
- **The catalog budget is finite and silent.** Claude Code empirically drops skills past a ~15.5–16k-character `available_skills` budget with only a "Showing X of Y" notice (gist by alexey-pelykh; claude-code#13099). Codex caps the catalog at 8,000 chars. Fusion-length descriptions (600–900 chars) are affordable for a family of 5–8 skills but would crowd out a 60-skill install. Front-load trigger keywords in the first ~50 chars. (A claimed `skillListingBudgetFraction` setting is unverified and disputed.)
- **Negative triggers are contested.** The house pattern uses them ("Do NOT use inside a Fusion bucket") and they demonstrably prevent cross-family misfires. Jesse Vincent's Superpowers 4 retro argues descriptions should carry only positive when-to-use because models act from the description without loading the body. These are compatible once you see the distinction: *cross-routing* negatives ("in context X use skill Y instead") earn their tokens; *behavioral* negatives ("don't write bad tests") belong in the body or nowhere.

## Distribution and the Supply Chain

The mid-2026 channels: **`npx skills add owner/repo`** (Vercel Labs, skills.sh — ~927K installs, ~71 harnesses, symlink-or-copy into every agent home) is the dominant open channel; **Claude Code plugin marketplaces** dominate inside the Claude ecosystem; npm-wrapped managers (skillpm — real semver + lockfiles; antfu/skills-npm) and the `skillet` Go binary are niche; install.sh + vendoring remains the self-owned path.

The dominant channels shipped with **no lockfile, no pinning, no signing** — and that gap is precisely what got exploited:

- **Snyk "ToxicSkills"** (Feb 2026): of 3,984 skills scanned on ClawHub and skills.sh, 36.8% had ≥1 security flaw, **76 confirmed malicious** (credential theft, backdoors); 91% of malicious skills combined prompt injection in the SKILL.md body with traditional malware; typosquatting throughout.
- **Koi Security "ClawHavoc"** (Feb 2026): 341 malicious ClawHub skills, 335 from one AMOS-stealer campaign hiding `curl | bash` in fake "Prerequisites" sections; count later grew to 824 as the registry grew.
- **OWASP now publishes an Agentic Skills Top 10**; AST02 (supply-chain compromise) prescribes signing, hash-pinned dependencies, and transparency logs. Academic work ("Semantic Supply-chain Attacks," arXiv 2605.11418) stresses that reviewing SKILL.md alone misses injection in content a skill fetches at runtime.

The house pattern anticipated all of this: Fusion pipes payloads through uv's hash-verified PyPI installs; Shaping Rooms' `vendor.lock.json` pins by git tree hash with an offline drift gate; PEP 723 scripts declare exact deps. Nothing in either repo fetches remote instructions at runtime.

## What Craftsman Dev Should Adopt

1. **The four-point skill contract, verbatim** — self-contained spec-strict skills, single-writer rule (`craftsman` CLI is the only thing that touches PLAN.md state, ledger trailers, verification records), a byte-identical `references/craftsman-conventions.md` traveling in every skill, one skill per accountability. This is the CLI–skill contract that makes "never ask an LLM whether code works" enforceable: skills judge, `craftsman verify` returns exit codes.
2. **Canonical home `~/.agents/skills` + project `.agents/skills`, adapters only where needed.** Port Fusion's `setup.py` agent table (link mode for Claude Code and Windsurf, standard mode for the rest), including attribution-checked never-destroy semantics and the sentinel/tree-digest provenance scheme. Update the table for Zed once its scan paths are verifiable.
3. **Thin bootstrap, tested brain.** `install.sh` stays under 100 lines and `exec`s `craftsman setup`; every placement decision lives in tested CLI code. Bundle `skills/craftsman-*` into the package at build time (hatch-hook pattern) so `craftsman update` refreshes CLI + skills in one verb and the repo `skills/` dir remains the single source of truth.
4. **The description formula**: identity → capabilities with safe default → quoted literal trigger phrases → cross-routing → applicability gate. Third person, trigger keywords in the first 50 characters, and — new discipline the prior projects didn't need — a **per-skill description budget (~500 chars)** so a full Craftsman install plus a user's other skills survives the ~16k catalog cutoff. Applied to Craftsman:

   ```yaml
   description: "Craftsman spec drafting — turn official docs into a
     Gherkin SPEC.md the human approves. Use for 'draft the spec',
     'write scenarios', 'spec this feature'. For batch planning use
     craftsman-plan; for implementation use craftsman-implement.
     Applies only inside a Craftsman project (AGENTS.md + craftsman
     CLI on PATH); otherwise offer `craftsman init` and stop."
   ```
5. **The lockfile discipline for anything vendored or distributed**: git-tree-hash pinning (`vendor.lock.json` + `check-vendor.sh` pattern) for any skill Craftsman vendors from elsewhere, and published sha256s for Craftsman's own releases. Distribute through the repo installer *and* a plugin-marketplace manifest; treat `npx skills add` compatibility as free (it reads any GitHub repo with spec-strict skills).
6. **AGENTS.md as the Instructions leg, CLAUDE.md as a symlink** — Shaping Rooms' "single tolerated harness artifact" rule, applied to every project Craftsman scaffolds.
7. **Skills-in-repos as the trust argument**: Craftsman's own supply-chain story (uv-verified installs, no runtime fetches, pinned PEP 723 deps, no `curl | bash` prerequisites inside skills) becomes a documented feature, not an accident — the ToxicSkills findings are the marketing copy.

## What NOT to Adopt

- **MCP servers as the skill–CLI bridge.** Amp allows skills to bundle MCP servers; the Triad's answer is a CLI on PATH — zero prompt cost, exit codes, works in every harness including ones with no MCP support. Prefer CLI; this is already doctrine.
- **Harness-specific frontmatter in portable skills.** Claude's `context: fork`/`arguments`, Cursor's `paths` — tempting, silently dead everywhere else. If a harness-specific optimization is ever worth it, it goes in a generated adapter layer (the gemini-gem-converter model), never in the canonical skill.
- **Marketplace-first distribution.** ClawHub-style registries are where the malware is; a methodology whose brand is verification cannot ship through channels with no pinning. Repo + installer is primary; marketplaces are mirrors.
- **Behavioral negative triggers in descriptions** ("do not write X"). Keep cross-routing negatives; move behavior into the body where the loaded skill governs.
- **Fusion's long descriptions at Craftsman's scale.** 900-char descriptions were fine for four skills; a 10–15-skill methodology family must budget harder or get silently truncated.
- **A skill package manager dependency** (skillpm, skillet, `npx skills`). Support them as consumers, depend on none: the installer must work with `sh`, `uv`, and `git` alone — the same austerity that makes the verification stack trustworthy.

## Conclusion

The house pattern is not just reusable — the ecosystem converged on it. Fusion bet in mid-2026 that `~/.agents/skills` plus spec-strict SKILL.md would become the portable substrate, and the harness matrix now confirms it: everything except Claude Code (symlink), Windsurf (copy), and Zed (unverified) reads the canonical directory natively. What Craftsman Dev inherits: the four-point contract, the five-move description formula, the thin-bootstrap/tested-brain installer with attribution-checked never-destroy semantics, and the tree-hash vendoring gate. What it must add: description token budgets (the silent ~16k catalog cutoff is new pressure the four-skill families never felt), a published-checksum release story, and an explicit supply-chain posture — because as of February 2026 the skill ecosystem has its own malware problem, and a methodology built on mechanical verification should be the easiest skill set in the world to trust mechanically.
