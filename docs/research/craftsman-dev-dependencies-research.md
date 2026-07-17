# Dependency & Supply Chain Management: Research & Architecture

> How the agent handles new dependencies, license auditing, vulnerability scanning, and supply chain integrity — evaluated against the 2026 landscape of agentic supply chain risks.

---

## The Problem

IBM's 2026 X-Force report documented a nearly 4× increase in supply chain compromises since 2020. The first CVE assigned to an agentic AI system (remote code execution via a crafted skill package) landed in January 2026. Snyk documented "ToxicSkills" — 1,184 malicious agent skill packages on skill registries. When an agent adds a dependency, it's extending the project's attack surface. Without a vetting protocol, the agent optimizes for "it works" without evaluating security, licensing, maintenance health, or duplication.

## The Dependency Vetting Protocol

Every new dependency introduced by the agent must pass a five-point check:

### 1. Known Vulnerabilities

```bash
npm audit                    # JavaScript/TypeScript
pip-audit                    # Python
cargo audit                  # Rust
swift package audit           # Swift (community tool)
```

A dependency with known HIGH or CRITICAL vulnerabilities is rejected. The agent finds an alternative or documents the accepted risk in an ADR.

### 2. License Compatibility

```bash
license-checker --production --failOn 'GPL-3.0;AGPL-3.0'   # JS
pip-licenses --fail-on 'GPL-3.0'                             # Python
cargo-license                                                 # Rust
```

The allowlist is defined in AGENTS.md. Typically: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC are allowed. GPL-3.0 and AGPL-3.0 require explicit human approval (they have viral licensing implications).

### 3. Maintenance Health

The agent checks before adding a dependency:
- Last commit date (older than 12 months → flag)
- Open issue count vs. closed ratio
- Number of maintainers (bus factor = 1 → flag)
- Whether the package is deprecated or archived

### 4. Duplication Check

Before adding a new dependency, the agent searches the existing codebase for equivalent functionality. A utility library that duplicates what an existing dependency already provides is a redundancy — and each dependency increases attack surface. Code intelligence tools (dependency-cruiser, CodeGraph) help identify existing alternatives.

### 5. AGENTS.md Compliance

If AGENTS.md includes an approved dependency list (for projects with strict supply chain requirements), the new dependency must be on the list or flagged for human approval.

## The Dependency Commit Convention

New dependencies get their own commit trailers:

```
chore(deps): add zod for runtime validation

Dependency: zod@3.22.4
License: MIT
Vulnerabilities: none (npm audit clean)
Justification: runtime type validation for API boundaries,
  replaces hand-written validators
Alternatives-considered: yup (larger bundle), joi (Node-only)
```

## Supply Chain as a Continuous Gate

Dependency auditing isn't a one-time check — it's a continuous gate. Known vulnerabilities are discovered after installation. The `craftsman security` gate runs `npm audit` / `pip-audit` / `cargo audit` at every batch boundary, catching new advisories against existing dependencies.

For Craftsman Dev, this means:
- **At dependency addition:** full five-point vetting protocol
- **At every batch boundary:** vulnerability scan against all dependencies
- **At finish:** full audit report with any accepted risks documented in ADRs

## Lockfile Integrity

Lock files (`package-lock.json`, `Pipfile.lock`, `Cargo.lock`, `Package.resolved`) are committed to git and verified at batch boundaries. Any modification to a lock file outside of an explicit dependency update is flagged — it could indicate supply chain tampering.

## Conclusion

Dependencies are attack surface. Every new dependency the agent introduces extends that surface. The five-point vetting protocol (vulnerabilities, license, maintenance, duplication, compliance) mechanizes what a senior developer does intuitively. The continuous audit at batch boundaries catches what changes after installation. The commit convention documents the rationale. The ADR system records accepted risks. No dependency enters the project without justification, and no known vulnerability ships without acknowledgment.
