# Dependency Vetting

Loaded when a batch or quick task is about to add a new dependency — run the five points, in order, before anything is installed.

Every dependency extends the project's attack surface. The five-point protocol below is the judgment layer above the `craftsman security` gate: the gate re-scans what is already installed at every boundary; you decide what gets installed at all. No point may be skipped, and the order matters — existence comes first because nothing else means anything against a package that isn't what you think it is.

## 1. Existence and slopsquatting — first, always

Hallucinated package names are a documented attack class: models emit plausible non-existent packages at scale, the fake names recur, and attackers register them (USENIX 2025: ~21.7% hallucinated-package rate in open models; 43% of fake names recur, making them squattable). Never install to find out. Query the registry first and read what comes back:

```bash
npm view <pkg> name version time.created   # JS/TS
pip index versions <pkg>                    # Python
cargo info <pkg>                            # Rust
# Swift: confirm the exact repo URL resolves and matches the vendor's official org
```

Reject on any of: the name does not exist; it exists but was created recently with negligible downloads while imitating a well-known name (`requets`, `zod-utils`); the registry entry's repository link does not match the project you meant. If the name came from your own memory or a doc snippet rather than the project's declared documentation sources, treat it as unverified until the registry confirms it.

## 2. Known vulnerabilities

```bash
npm audit          # JS/TS
pip-audit          # Python
cargo audit        # Rust
osv-scanner --lockfile=<lockfile>   # any stack, incl. Swift Package.resolved
```

Run the scan with the candidate added (in a throwaway resolution, not a committed install). HIGH or CRITICAL → reject and find an alternative; if no alternative exists, the accepted risk is an ADR and a human approval, never a silent install.

## 3. License against the AGENTS.md allowlist

Check the package's license — and its transitive licenses — against the allowlist in AGENTS.md (typically MIT, Apache-2.0, BSD-2/3-Clause, ISC):

```bash
license-checker --production --failOn 'GPL-3.0;AGPL-3.0'   # JS/TS
pip-licenses --fail-on 'GPL-3.0;AGPL-3.0'                   # Python
cargo license                                                # Rust
```

GPL-3.0 or AGPL-3.0 anywhere in the tree → stop and get explicit human approval; viral licensing is a business decision, not yours. No allowlist in AGENTS.md → ask the human to declare one before proceeding.

## 4. Maintenance health

Check the registry metadata and repository: last release older than ~12 months → flag; a single maintainer (bus factor 1) on a load-bearing package → flag; deprecated or archived → reject outright. Example: `npm view <pkg> time.modified maintainers deprecated` answers all three for JS in one command. A flag is not an automatic rejection — it is a line in the justification, weighed against how load-bearing the dependency will be.

## 5. Duplication — search what you already have

Before adding capability, prove the tree doesn't already provide it:

```bash
npm ls --all | grep -i <capability>    # or: read package.json deps
uv tree | grep -i <capability>          # Python
cargo tree | grep -i <capability>       # Rust
```

An existing dependency (or the standard library) that covers the need wins over a new package, even a "better" one — each addition is attack surface and a future audit line. Example: the project already has `zod` → no `yup`, no `joi`, no hand-rolled validator.

## Recording the decision

Commit the addition alone — `chore(deps)`, never mixed with the feature or a fix — through `craftsman commit`, with a `Dependency:` trailer carrying the vetting result:

```
chore(deps): add zod for runtime boundary validation

Dependency: zod@3.23.8, MIT, npm audit clean, registry-verified
  (published 2020, 12M weekly downloads); validates API request
  bodies at the boundary — replaces hand-written validators.
Alternatives-considered: yup (larger bundle), joi (no static inference)
```

**Lockfile integrity**: `package-lock.json`, `uv.lock`, `Cargo.lock`, `Package.resolved` change only through an explicit dependency operation you just performed and are about to commit as `chore(deps)`. A lockfile diff appearing in any other commit is a tampering signal — revert it and investigate before continuing.

## When the decision needs an ADR

Draft an ADR (human-gated, per conventions) before adding when the dependency is:

- **Transitive-heavy** — it pulls a large subtree (`cargo tree | wc -l` jumps by dozens); you are vouching for every node.
- **Security-sensitive** — crypto, auth, session handling, or parsing untrusted input; record why this one and what was rejected.
- **Framework-shaping** — it wants to own control flow or architecture (effect-ts, TCA-class libraries): adopting it is an architectural commitment, and so is removing it.

A leaf utility with a clean five-point pass needs only the trailer. Everything above that threshold needs the reasoning written down where the next session will find it.
