# The Documentation Grounding Pipeline: MCPs, CLIs, and Offline Doc Files

> How to mechanize "official documentation driven" across every target stack — the mid-2026 landscape of documentation MCP servers, the llms.txt standard, offline doc formats, and the CLI-first design for a `craftsman docs` subcommand that gives every harness the same grounding.

---

## The Question

"Official documentation driven" is Craftsman Dev's non-negotiable constraint: the agent never relies on training data for API surfaces. Prior research established *that* Context7-style pipelines can mechanize this. This document goes deeper: what is the actual state of doc-grounding infrastructure in July 2026, per stack, and what should the `craftsman` CLI build on — given that MCP support varies by harness and the skill set must stay agent-agnostic?

All version numbers, prices, and endpoint states below were verified against live sources on 2026-07-17. Items that could not be verified are marked **UNVERIFIED**.

## The Documentation MCP Landscape (Mid-2026)

The market has split into three tiers: **aggregators** (Context7, Ref, Nia), **first-party official servers** (Microsoft, AWS, Cloudflare, Apple-via-Xcode), and **self-hosted indexers** (Docs MCP Server). The trend is clear: official first-party servers are displacing generic scrapers for their own platforms.

| Server | Status (2026-07) | Auth | Cost | Coverage | Offline |
|---|---|---|---|---|---|
| **Context7** (Upstash) | Very active (59k stars, release today) | Optional key | Free: 1k calls/mo; Pro $10/seat/mo (5k calls); private docs $25/1M tokens | Broad OSS + private (Pro) | No |
| **Docs MCP Server** (arabold) | Active, v2.4.2 | Self-hosted | Free (optional embedding API) | Anything you index: web, GitHub, npm, PyPI, PDFs, local files | **Yes** (Ollama embeddings) |
| **Microsoft Learn MCP** | GA, official | **None** | Free | All MS Learn + code samples | No |
| **AWS Docs MCP** (awslabs) | Active; remote AWS MCP Server GA May 2026 | None (local docs server) | Free | All AWS docs | No |
| **Cloudflare Docs MCP** (docs.mcp.cloudflare.com) | Live, official | **None** | Free | All CF docs (Vectorize semantic search) | No |
| **sosumi.ai** (NSHipster) | Healthy, active | None | Free (unofficial; unpublished rate limits) | Apple docs + WWDC transcripts + any DocC site | No |
| **Xcode 26.3 MCP** (`xcrun mcpbridge`) | Shipped March 2026, official | Local | Free | Apple docs corpus + WWDC via `DocumentationSearch` | Partially (local embeddings) |
| **DeepWiki MCP** (Cognition) | Live, free | None | Free (public repos) | AI-*inferred* wikis of any GitHub repo | No |
| **GitMCP** (gitmcp.io) | Working but coasting (quiet since May) | None | Free | Per-repo llms.txt → README | No |
| **Ref** (ref.tools) | Active; pricing moved upmarket | Key required | Free 200 one-time credits; $19–200/mo | Broad + private docs/PDFs | No |
| Rust community MCPs (snowmead/rust-docs-mcp et al.) | Fragmented, none dominant | Varies | Free | crates.io / docs.rs | Some cache locally |

Key facts worth internalizing:

- **Context7 is no longer free-unlimited.** Free tier is 1,000 calls/month with a +20/day trickle. It now has a **public REST API** (`GET /api/v2/libs/search`, `GET /api/v2/context`, Bearer key) that works via plain curl — verified unauthenticated at low shared rate limits — and an **official `ctx7` CLI** (`npx ctx7`) whose `setup` command offers an explicit **"CLI + Skills, no MCP" mode**. Upstash itself now endorses the CLI-over-MCP pattern.
- **Context7 had a real injection incident**: Noma Security's "ContextCrush" disclosure showed library-owner "AI Instructions" were served verbatim into agent context, and trust signals were gameable. Upstash responded with a two-pass injection-detection pipeline and quality scoring. Lesson: third-party doc aggregators are an injection surface; treat fetched snippets as data, never instructions.
- **Cloudflare Docs MCP** exposes `search_cloudflare_documentation` keylessly and is excellent for its platform. Cloudflare also shipped "Docs for agents" and a code-mode API MCP (~1,000 tokens of tool surface vs ~1.17M naive) — the token-efficiency benchmark for the space.
- **Apple finally has official agent doc access — but only through Xcode.** Xcode 26.3 turned Xcode into an MCP server (`xcrun mcpbridge`, ~20 tools) including `DocumentationSearch` over the full Apple docs corpus + WWDC transcripts with local MLX embeddings. Requires Xcode running with a project open; Mac-only. For everything else there's sosumi.ai (URL-swap `developer.apple.com` → `sosumi.ai`, plus MCP, CLI, and an installable SKILL.md), and community `apple-docs-mcp` variants (kimsungwhee's is feature-rich but slowing; MightyDillah's is active).
- **Swift Package Index has no MCP or llms.txt initiative** that could be found (**UNVERIFIED** that none is in progress). sosumi's `fetchExternalDocumentation` renders any DocC-hosted site (including SPI docs) to markdown — the practical gap-filler.
- **DeepWiki is a comprehension tool, not ground truth**: wikis are AI-inferred, freshness lags repo HEAD (cadence undocumented), and 2026 guides explicitly warn against treating it as authoritative. Useful for architecture orientation of an unfamiliar dependency; never for API surfaces.
- **Rust and Python MCPs are fragmented community efforts** with no dominant, durable option — which strengthens the case for the file/CLI path in those stacks (docs.rs and objects.inv are better foundations than any current MCP).

## llms.txt: Dead as a Crawler Standard, Alive as an Agent Sitemap

The data is unambiguous. Ahrefs' May 2026 study of 137k domains: 28% publish an llms.txt, but **97% of those files received zero requests**, and AI retrieval bots accounted for ~1.1% of the traffic that did arrive. No AI crawler probes for it; Google compares it to the keywords meta tag. As a GEO/crawler standard, llms.txt failed.

But as an **on-demand sitemap for coding agents**, it works — and adoption among developer-tool docs is near-universal (Mintlify made it platform-default). Verified live on 2026-07-17:

| Publisher | llms.txt | llms-full.txt | Per-page `.md` |
|---|---|---|---|
| Cloudflare | Yes (15.6 KB index + per-product files) | Yes — **57.8 MB** | Yes (`/workers/index.md`) |
| Anthropic (platform.claude.com) | Yes (~1,894 pages) | Yes — **90.75 MB** | Yes |
| Stripe | Yes — includes *prescriptive agent instructions* | — | — |
| AWS | Yes (295 KB, per-guide) | — | Yes |
| Next.js / Svelte / Hono / React / Vue | Yes (Svelte offers small/medium/full tiers) | Yes | Yes (Next.js) |
| Apple, Microsoft Learn, docs.python.org, doc.rust-lang.org, Google Cloud | **404 — none** | — | — |

Consuming tooling exists and is healthy: **llms_txt2ctx** (Answer.AI's original CLI — parses llms.txt into an XML context document), **mcpdoc** (LangChain's minimal MCP that serves any user-declared llms.txt list with domain-restricted fetching; actively maintained), and generator plugins for VitePress, Docusaurus, and Nuxt. Stripe's llms.txt is notable for containing *prescriptive agent instructions* ("prefer Checkout Sessions over the Charges API") — publishers are starting to treat the file as a steering surface, not just an index. That cuts both ways: it is useful guidance and an injection vector, and the pipeline should treat it as untrusted data like any fetched content.

Three conclusions for `craftsman docs`:

1. **llms-full.txt is unusable as context** at platform scale (57–90 MB). It is a corpus for local indexing/grep, not a prompt payload. Svelte's tiered small/medium/full approach is the sane variant.
2. **The per-page `.md` convention is the durable half of the standard.** `curl https://developers.cloudflare.com/workers/index.md` returns clean markdown. Mintlify adds `Accept: text/markdown` content negotiation across thousands of hosted doc sites. This is the cheapest possible grounding fetch: no MCP, no key, no tokens wasted on HTML.
3. **Platform vendors (Apple, Microsoft, Google, Python, Rust core) don't publish llms.txt** — exactly the stacks where first-party MCPs or offline files must fill the gap. llms.txt should be a first-class *source type* in `craftsman docs`, never the only one.

## Official Documentation as Files (Offline/Local)

The strongest 2026 trend: doc toolchains are growing agent-grade output formats natively.

| Stack | Format | Agent-queryable how | 2026 status |
|---|---|---|---|
| Apple/Swift | `.doccarchive` (render JSON under `data/documentation/`) | **`docc convert --enable-experimental-markdown-output`** emits per-page `.md` (Swift 6.3, March 2026 — explicitly for LLMs) | Experimental but shipped |
| OSS Swift | `swift package generate-documentation` (swift-docc-plugin 1.4.5) | Same markdown flag; grep the output | Active |
| Rust | rustdoc JSON (`--output-format json`) | **Still unstable** (nightly `-Z unstable-options`; tracking #76578 open six years after RFC). But **docs.rs serves prebuilt rustdoc JSON** per crate; `cargo-doc-md` / `rustdoc-md` convert to LLM-grade markdown; `rustup doc` = full offline std docs | JSON unstable; ecosystem thriving anyway |
| Python | Sphinx `objects.inv` | `sphobjinv suggest/convert` (v2.4, March 2026) fuzzy-searches any Sphinx inventory; `python -m pydoc`; downloadable docs.python.org bundles | Active |
| TypeScript | `.d.ts` in node_modules | **The ground truth is already vendored** — `tsc` checks against exactly these files; `typedoc-plugin-markdown` v4.12 (June 2026) for readable exports; api-extractor `.api.md` reports | Active |
| bash/CLI | man pages, `--help`, tldr | `man -k` / `apropos`; official Rust client `tlrc`; `--help` is version-exact ground truth | Healthy |
| Any | Dash docsets | `docSet.dsidx` is **plain SQLite** — `sqlite3` queries work; `dasht` CLI; Kapeli feeds maintained (202 official + 316 contributed); **DocSetQuery** (new, Paul Solt) exports Apple's docset to markdown "for agent workflows" | Format stable; Apple docset moved to brotli chunks |

What "the agent queries these locally" looks like, concretely — every one of these is a plain shell command available to any harness:

```bash
# Apple/Swift: export a dependency's DocC as markdown, then grep it
swift package generate-documentation --target NIOCore \
  --enable-experimental-markdown-output --output-path .craftsman/docs/swift-nio
rg "channelRead" .craftsman/docs/swift-nio

# Rust: fetch docs.rs prebuilt rustdoc JSON, convert, query; std docs fully offline
curl -sL https://docs.rs/crate/tokio/latest/json | gunzip > tokio.json
cargo doc-md            # cargo-doc-md: rustdoc JSON -> per-module markdown
rustup doc --std --path # print local path to installed std documentation

# Python: fuzzy-search any Sphinx site's API inventory without downloading the docs
sphobjinv suggest https://docs.pydantic.dev/latest/objects.inv model_validator -su

# TypeScript: the ground truth is already on disk, version-exact
cat node_modules/hono/dist/types/hono.d.ts
npx typedoc --plugin typedoc-plugin-markdown --out .craftsman/docs/self src/index.ts

# Dash docsets: the index is plain SQLite — no app needed
sqlite3 ~/Docsets/Redis.docset/Contents/Resources/docSet.dsidx \
  "SELECT name, type, path FROM searchIndex WHERE name LIKE '%EXPIRE%'"

# bash: version-exact ground truth for the installed tool
man -k compress; tldr tar; jq --help
```

**Vendored docs-in-repo** (`ai_docs/`, `docs/vendor/`): prior art is real — IndyDevDan's `ai_docs/` pattern, Aider's `CONVENTIONS.md` (`--read`, prompt-cacheable), community `.ai/docs/` variants. Two hygiene rules emerged from research:

- **Licensing**: vendoring API docs *generated from your own dependencies' source* (docc, rustdoc, typedoc output) is low-risk. Vendoring Apple/MSDN prose into a **public** repo is not — Apple docs are proprietary. Local caches outside version control (or private repos) are the safe pattern.
- **Staleness**: no named community convention exists (**UNVERIFIED** as standard practice); the workable mechanism is pinning vendored docs to lockfile versions and regenerating on dependency bumps — which a CLI can automate.

## The CLI-over-MCP Path: `craftsman docs`

The ecosystem validated the Triad Pattern's bet during 2025–2026. Upstash shipped `ctx7` with a no-MCP skills mode; Microsoft shipped `@microsoft/learn-cli`; sosumi ships a CLI and a SKILL.md; Mintlify made bare `curl` the tool by supporting `Accept: text/markdown` server-side. Widely-cited comparisons claim CLI-based agents beat MCP-based ones by 10–32x on tokens (methodology **UNVERIFIED**, but the direction matches Cloudflare's own code-mode numbers).

Design sketch, composing verified primitives:

```bash
craftsman docs add hono --source llms.txt --url https://hono.dev/llms.txt
craftsman docs add cloudflare-workers --source mcp --endpoint https://docs.mcp.cloudflare.com/mcp
craftsman docs add swift-nio --source docc          # builds + markdown-exports the dep's DocC
craftsman docs add tokio --source docsrs-json       # prebuilt rustdoc JSON → markdown
craftsman docs sync                                  # fetch/regenerate all, pin to lockfile versions
craftsman docs search "streaming response" --lib hono   # ripgrep + inventory search over local cache
craftsman docs get hono/routing                      # print one page as markdown to stdout
```

Resolution order per library: (1) declared source in AGENTS.md → (2) per-page `.md` / llms.txt probe → (3) registry docs (docs.rs JSON, npm `.d.ts`, objects.inv) → (4) Context7 API as aggregator fallback → (5) fail loudly and ask the human. Every harness — Claude Code, Codex, Cursor, Gemini CLI — gets identical grounding through a shell command; MCP becomes an optional accelerator, not a dependency. The cache layout:

```
.craftsman/docs/            # gitignored by default (licensing + size)
├── manifest.json           # library -> {source, url, version, fetched_at, lockfile_ref}
├── hono@4.6.14/            # keyed by library@version, from the lockfile
│   ├── llms.txt            # the index as fetched
│   └── pages/*.md          # per-page markdown
├── tokio@1.45.0/
│   ├── rustdoc.json        # docs.rs prebuilt JSON (upstream artifact)
│   └── md/                 # cargo-doc-md conversion, what the agent greps
└── swift-nio@2.81.0/
    └── md/                 # DocC markdown export
```

`craftsman docs sync` diffs `manifest.json` against the project lockfiles (package-lock.json, Cargo.lock, Package.resolved, uv.lock) and refetches only what moved — staleness becomes a mechanical check (`craftsman docs status` lists drift) instead of a hope. The companion skill is thin: "before writing code against any library, run `craftsman docs search <query> --lib <name>`; if the library is unknown, run `craftsman docs add` or stop and ask." That is the whole prompt-side cost — the Triad Pattern working as intended.

### The AGENTS.md "Documentation Sources" Section (Proposed Spec)

```markdown
## Documentation Sources
<!-- The agent MUST consult these before writing code against any listed surface.
     Precedence: first matching entry wins. `verify` names the mechanical gate. -->

| Library / Surface | Source | Location | Pinned | Verify |
|---|---|---|---|---|
| Cloudflare Workers | mcp | https://docs.mcp.cloudflare.com/mcp | runtime 2026-07 | wrangler types + tsc |
| Hono | llms.txt | https://hono.dev/llms.txt | 4.x | tsc |
| Apple SwiftUI | xcode-mcp \| sosumi | xcrun mcpbridge; https://sosumi.ai/ | iOS 26 SDK | swiftc @available |
| swift-nio (dep) | docc | .craftsman/docs/swift-nio/ | lockfile | swift build |
| tokio (dep) | docsrs-json | .craftsman/docs/tokio/ | Cargo.lock | cargo check + cargo-semver-checks |
| Internal billing API | file | docs/vendor/billing-openapi.yaml | v3 (2026-06-12) | contract tests |

Fallback aggregator: context7 (API key in env). Unlisted library → STOP and ask.
```

Format rationale: a table is greppable by both humans and the CLI; `Source` is a closed enum (`mcp | llms.txt | docc | docsrs-json | objects.inv | dts | file | context7`); `Pinned` makes staleness auditable; `Verify` ties each source to its mechanical gate (next section). The final line encodes the librarian rule: no source, no code.

## Verifying Doc Grounding Mechanically

Can the pipeline *detect* code written against a stale API? Largely yes, in layers:

1. **Type checkers are the real gate.** `tsc` against vendored `.d.ts`, `swiftc`, `cargo check`, and pyright catch *nonexistent* APIs mechanically — the compiler is the first reviewer. Caveat from 2026 research: they catch fantasy APIs, not semantically wrong use of real ones. That residue is what Gherkin verification covers.
2. **Deprecation linting is now typed and mechanical**: `@typescript-eslint/no-deprecated` (successor to eslint-plugin-deprecation, in `strict-type-checked`); Python PEP 702 `@warnings.deprecated` enforced by pyright `reportDeprecated` / mypy `--enable-error-code deprecated`; Swift `@available(*, deprecated)`; Rust `#[deprecated]` + clippy. "Code uses deprecated API" becomes a lint failure, not a review comment.
3. **API diff tools** close the loop when dependencies bump: `swift package diagnose-api-breaking-changes` (swift-api-digester), **cargo-semver-checks** (v0.48, 245 lints, official 2026 project goal to merge into cargo), api-extractor `.api.md` reports (diffable baselines), **griffe check** for Python. `craftsman docs sync` after a dependency bump can run the relevant differ and hand the agent a machine-generated "what changed" report.
4. **Hallucinated-package screening**: slopsquatting is a named threat class (USENIX 2025: ~21.7% hallucinated-package rate in open models; 43% of fake names recur, making them registerable by attackers). Before any new dependency: verify existence and health on the registry (Socket.dev, `npm view`, `cargo info`, PyPI/OSV via mcp-pypi). This belongs in the dependency skill, gated by the same pipeline.

## What Craftsman Dev Should Adopt

1. **CLI-first, MCP-optional — now with ecosystem proof.** Build `craftsman docs` on curl-able primitives (per-page `.md`, llms.txt, Context7 REST v2, docs.rs JSON, sphobjinv, local `.d.ts`/DocC). Register first-party MCPs (Cloudflare Docs, Xcode, MS Learn) as accelerators where the harness supports them.
2. **The AGENTS.md Documentation Sources table** as specified above — the human declares sources once; the agent and CLI both parse the same table; unlisted library means stop and ask.
3. **Per-page `.md` probing as the default fetch**: try `<url>.md` / `Accept: text/markdown` before anything heavier. Zero cost, zero auth, growing server-side support.
4. **Version-pinned local doc cache** (`.craftsman/docs/`, gitignored), regenerated from lockfiles: DocC markdown export for Swift deps, docs.rs JSON → markdown for Rust, `.d.ts` as-is for TypeScript, objects.inv-guided fetches for Python.
5. **Tie every source to a mechanical verify gate**: typed deprecation lints on by default in `craftsman lint`; the relevant API differ (cargo-semver-checks, diagnose-api-breaking-changes, griffe, api-extractor) on dependency bumps; registry-existence checks for new packages.
6. **Treat fetched docs as data, not instructions.** The ContextCrush incident is the concrete precedent: aggregator snippets can carry injected directives. The skill should state it plainly.

## What NOT to Adopt

- **DeepWiki/GitMCP as grounding sources.** DeepWiki is AI-inferred (useful for orientation, explicitly not ground truth); GitMCP is single-maintainer and coasting. Neither meets the "official documentation" bar.
- **llms-full.txt as context payload.** 57–90 MB files are corpora, not prompts. Ingest into the local cache; never inject wholesale.
- **A self-hosted embedding index (Docs MCP Server) as the default.** Excellent for teams with private corpora; for a solo craftsman it adds an embedding provider, a service, and index maintenance where ripgrep over version-pinned markdown suffices. Keep it as a documented option for private-docs-heavy projects.
- **Paid aggregators as a hard dependency.** Context7's free tier (1k calls/mo) and Ref's upmarket pricing ($19+/mo, key required) make them fine *fallbacks* but wrong *foundations* for a portable methodology. The pipeline must work with zero accounts.
- **Vendoring proprietary doc prose into public repos.** Apple/MSDN content is not redistributable; cache locally, don't commit.

## Conclusion: The Recommended Per-Stack Pipeline

| Stack | Primary source | Best MCP (if harness supports) | CLI/file fallback (always works) | Verify gate |
|---|---|---|---|---|
| **Apple Swift** | Apple docs via Xcode 26.3 `DocumentationSearch` | Xcode MCP (`xcrun mcpbridge`); sosumi.ai MCP off-Mac/headless | sosumi URL-swap or CLI; DocSetQuery over the Dash docset | swiftc + `@available` warnings |
| **OSS Swift** | Package's own DocC | sosumi `fetchExternalDocumentation` (renders any DocC site) | `docc convert --enable-experimental-markdown-output` into local cache | swift build + diagnose-api-breaking-changes |
| **Python** | Library's Sphinx/RTD docs | (no dominant MCP — skip) | `sphobjinv suggest` + per-page fetch; pydoc for stdlib | pyright/mypy + PEP 702 deprecated |
| **TypeScript** | **`.d.ts` in node_modules** (already local, already versioned) | Context7 for guides/examples | `tsc`-checked declarations; typedoc-markdown export; llms.txt (most TS frameworks publish one) | tsc + `no-deprecated` |
| **Rust** | docs.rs | (fragmented — skip) | docs.rs prebuilt rustdoc JSON → cargo-doc-md; `rustup doc` offline std | cargo check + cargo-semver-checks |
| **bash** | man + `--help` | — | `man -k`, `tlrc`; `--help` is version-exact | shellcheck; `--help` diff |
| **Cloudflare** | developers.cloudflare.com | **Cloudflare Docs MCP** (keyless, official — keep using it) | per-page `index.md` + per-product llms.txt via curl | `wrangler types` + tsc |
| **Anything else** | Human-declared in AGENTS.md | Context7 (free tier) | `ctx7` CLI / Context7 REST v2; llms.txt probe | stack type checker |

The architecture in one sentence: **declare sources in AGENTS.md, fetch through a CLI that prefers free curl-able official sources and caches version-pinned markdown locally, use first-party MCPs as accelerators where the harness allows, and let type checkers, deprecation lints, and API differs prove the grounding held.** The librarian keeps its rule — no documentation, no code — but the rule now has infrastructure.

---

*Key sources (all checked 2026-07-17): github.com/upstash/context7 · context7.com/plans, /docs/api-guide, /docs/clients/cli · noma.security (ContextCrush disclosure) · upstash.com/blog/context7-quality-and-safety · github.com/arabold/docs-mcp-server · learn.microsoft.com/en-us/training/support/mcp · awslabs.github.io/mcp · developers.cloudflare.com/agents/model-context-protocol/ and /docs-for-agents/ · sosumi.ai · developer.apple.com/documentation/xcode/giving-external-agents-access-to-xcode · swift.org/blog/swift-6.3-released (DocC markdown output) · rust-lang/rust#76578 · docs.rs/about/rustdoc-json · github.com/bskinn/sphobjinv · api.rushstack.io/pages/api-extractor · typescript-eslint.io/rules/no-deprecated · peps.python.org/pep-0702 · github.com/obi1kenobi/cargo-semver-checks · mkdocstrings.github.io/griffe · ahrefs.com/blog/llmstxt-study (llms.txt traffic data) · llmstxt.org · github.com/langchain-ai/mcpdoc · docs.devin.ai/work-with-devin/deepwiki-mcp · github.com/idosal/git-mcp · docs.ref.tools/usage/pricing · kapeli.com/docsets · github.com/PaulSolt/DocSetQuery · USENIX Security 2025 slopsquatting study via CSA research note 2026-04.*
