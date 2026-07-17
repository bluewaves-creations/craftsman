# Security as a Mechanical Gate: Research & Architecture

> SAST, secret scanning, dependency auditing, and supply chain security as deterministic verification gates — evaluated against the 2026 landscape of AI-generated code vulnerabilities and agentic security.

---

## The Problem

A 2025 study found 45% of AI-generated code contains security vulnerabilities in the initial version. Java implementations show failure rates exceeding 70%. The agent optimizes for functionality — making the test pass — not for security. Without a mechanical gate, vulnerabilities ship silently.

The 2026 landscape has made this more urgent: IBM's X-Force report documented a nearly 4× increase in supply chain compromises since 2020. The first CVE assigned to an agentic AI system (remote code execution via a crafted skill package) landed in January 2026. Security is no longer a periodic audit — it's a per-commit gate.

## The Security Verification Stack

Three parallel gates, each catching a different class of vulnerability:

### Gate 1: Secret Scanning

Prevents credentials, API keys, tokens, and private keys from entering the repository.

Tools: git-secrets, gitleaks, truffleHog, GitHub secret scanning (built-in). All are deterministic — pattern matching against known secret formats.

For Craftsman Dev: runs as a pre-commit hook. A detected secret fails the commit immediately. The agent never commits secrets, but mechanical enforcement catches mistakes the agent doesn't realize are secrets (connection strings, embedded tokens in config files).

### Gate 2: Static Application Security Testing (SAST)

Analyzes source code for insecure patterns: SQL injection, XSS, path traversal, insecure deserialization, hardcoded credentials.

Tools by ecosystem: Semgrep (multi-language, open-source, fastest growing), CodeQL (GitHub-native, deep data-flow analysis), Bandit (Python), eslint-plugin-security (JavaScript), SwiftLint security rules (Swift), cargo-audit + clippy (Rust).

Semgrep is the pragmatic default for multi-stack projects: one tool, one config, covers Python, TypeScript, Java, Go, Ruby, Swift. Custom rules can enforce project-specific security patterns from AGENTS.md.

For Craftsman Dev: runs at batch boundaries alongside lint and health checks. A SAST finding at severity HIGH or CRITICAL fails the batch. The agent fixes the vulnerability before reporting the batch as complete — same improvement loop as any other gate failure.

### Gate 3: Dependency Auditing (SCA)

Checks all dependencies against known vulnerability databases (NVD, GitHub Advisory Database, OSV).

Tools: npm audit (JavaScript), pip-audit (Python), cargo-audit (Rust), OWASP Dependency-Check (multi-language). All are deterministic — database lookup, not opinion.

For Craftsman Dev: runs at batch boundaries. A known vulnerability in a dependency at severity HIGH or CRITICAL fails the batch. The agent either upgrades the dependency, finds an alternative, or documents the accepted risk in an ADR.

## Dependency Vetting Protocol

When the agent introduces a new dependency, it must vet it before adding it:

```
Agent proposes new dependency
    │
    ├── 1. Check known vulnerabilities (npm audit / pip-audit / cargo-audit)
    ├── 2. Check license compatibility (license-checker / cargo-license)
    ├── 3. Check maintenance status (last commit, open issues, bus factor)
    ├── 4. Check download/usage statistics (community adoption)
    ├── 5. Verify against AGENTS.md approved dependencies (if listed)
    │
    └── If any check fails → flag to human before proceeding
```

New dependencies require explicit justification. The commit message includes a `Dependency:` trailer with the rationale. For significant new dependencies, an ADR records the evaluation.

## Integration with craftsman check-all

```bash
craftsman verify       # Gherkin scenarios (functional)
craftsman lint         # code style + token compliance
craftsman arch         # fitness functions (architectural)
craftsman security     # secret scan + SAST + dependency audit (parallel)
craftsman health       # CodeScene structural quality
craftsman a11y         # axe-core WCAG scan (front-end)
craftsman visual       # Playwright screenshot regression (front-end)
```

The three security scans run in parallel — they're independent. Total wall-clock time: the slowest of the three (typically SAST at ~2-5 minutes), not the sum.

## Conclusion

Security is a mechanical gate, not an audit. Secret scanning, SAST, and dependency auditing are deterministic tools that return pass/fail — exactly like Gherkin scenarios, lint rules, and fitness functions. They belong in the same verification stack, running at the same batch boundaries, with the same improvement loop when they fail. The agent fixes the vulnerability the same way it fixes a failing scenario: read the error, consult the docs, apply the minimal fix, verify green.
