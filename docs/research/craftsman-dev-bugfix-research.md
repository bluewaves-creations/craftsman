# High-Quality Bug Fixing: Research & Alternatives

> Thorough bug fixing without patching, with codebase quality preservation and principled refactoring — evaluated against the 2026 landscape of systematic debugging, refactoring runaway, and code health–guided repair.

---

## The Question

When an agent encounters a bug — whether a red Gherkin scenario, a regression, or a production report — how should it fix it without degrading codebase quality? The default agent behavior is to patch: make the test pass by whatever means necessary, even if that means adding special-case logic, duplicating code, or introducing coupling. How does Craftsman Dev enforce thorough, root-cause fixing that leaves the codebase *better* than it found it?

## The Core Failure Modes

### 1. Patch Over Root Cause

The most common agent bug-fixing failure: the agent makes the failing test pass without understanding *why* it was failing. A null pointer exception gets wrapped in a try-catch. A race condition gets masked with a sleep(). A missing validation gets handled at the call site instead of the boundary. The test turns green. The root cause remains.

The systematic debugging research gives this a protocol name: **"diagnose before fix."** The agent must reproduce, isolate, and hypothesize before writing a single line of fix code. The Hermes agent skill enforces this with a four-phase framework:

1. **Root Cause Investigation** — read the error, reproduce consistently, check recent changes, trace data flow
2. **Pattern Analysis** — compare working vs. broken, list every difference
3. **Hypothesis Testing** — single clear hypothesis, smallest possible change to verify
4. **Implementation** — write a failing test for the root cause, fix the root cause, verify green, verify no regressions

The critical constraint: **Phase 4 must create a failing test *for the root cause*, not for the symptom.** If the test only catches the symptom, the root cause can recur through a different code path.

### 2. Refactoring Runaway

A May 2026 paper ("Refactoring Runaway") analyzed 3,691 patches from coding agents and found that **tangled refactoring** — mixing bug fixes with unrelated structural changes — occurs in 21.43% of agent-generated patches. While this is actually less frequent than human developers (36.72%), the consequences are worse: tangled refactorings are strongly associated with reduced compilability.

The paper's recommendation for framework designers:

1. **Restrict large-scale refactorings unless explicitly required** by the issue context
2. **Validate method-signature modifications** against inheritance hierarchies and interface contracts
3. **Separate refactoring-related transformations from bug-fix generation** into independently verifiable stages

This is the critical insight for Craftsman Dev: **fix and refactor are two separate commits, never one.** The fix addresses the root cause and does nothing else. The refactoring improves the surrounding code and does nothing else. Each is independently verifiable. If the refactoring introduces a regression, you revert it without losing the fix.

### 3. Quality Degradation Through Fixes

CodeScene's research quantifies the problem: agents consume **50% more tokens** and introduce **60% more defects** when working on unhealthy code. Each patch that degrades code health makes the *next* fix harder and more error-prone. This creates a compound decay: patches make code unhealthier, unhealthy code makes patches worse, worse patches make code even unhealthier.

The solution is **code health as a gate on every fix.** CodeScene's PR Refactoring Agent enforces this: it "limits itself to new degradations and avoids modifying previously existing issues elsewhere in the codebase." If your fix introduces new code health issues, you must refactor those issues before the fix is accepted — but you don't refactor unrelated pre-existing issues in the same change.

## The Three-Phase Fix Protocol

### Phase 1: Diagnose (Never Skip)

The agent must not touch code until the root cause is identified.

```
Bug report or red scenario
    │
    ├── 1. Reproduce
    │   └── Can you trigger the failure with a single command?
    │       └── If not → gather more data, don't guess
    │
    ├── 2. Isolate
    │   ├── git bisect to find the introducing commit
    │   ├── Binary search: comment out code sections
    │   └── Trace data flow from input to failure point
    │
    ├── 3. Hypothesize
    │   └── Single clear hypothesis about the root cause
    │       └── What is the smallest change that would verify this?
    │
    └── 4. Report (before fixing)
        ├── Root cause identified: [description]
        ├── Introducing commit: [hash] (if applicable)
        ├── Affected scope: [files, modules]
        └── Proposed fix approach: [description]
```

The report-before-fix step is the human gate. For critical bugs, the human reviews the diagnosis before the agent implements. For routine bugs, the agent proceeds to Phase 2 but the diagnosis is recorded in the commit.

### Phase 2: Fix (Minimal, Root-Cause Only)

The fix commit does exactly three things:

1. **Write a failing test for the root cause** — not the symptom. The test should fail before the fix and pass after it. It should be specific enough that this exact root cause can never recur without detection.

2. **Apply the minimal fix** — address the root cause and nothing else. No refactoring. No "while I'm here" improvements. No unrelated changes.

3. **Verify** — run full test suite, not just the new test. The fix must not introduce regressions.

```
fix(batch-2): handle expired JWT tokens in middleware

Root-cause: middleware checked token validity but not expiry,
allowing expired tokens to pass validation.

Scenarios: "Reject expired token" now passing
Ref: SPEC.md scenario "Reject expired token"
Verified-by: craftsman verify --all
Previously-failed: none
```

### Phase 3: Improve (Separate Commit, Quality Gate)

After the fix is green and committed, the agent evaluates whether the surrounding code needs improvement. This is a *separate* operation:

1. **Run code health review** on the files touched by the fix
2. **If health score decreased** → refactor to restore or improve health (mandatory)
3. **If health score is already low** (< 8.0) → refactor to improve (recommended, not mandatory)
4. **Refactor in a separate commit** with its own verification

```
refactor(batch-2): extract token validation into dedicated module

Moved token validation from middleware to TokenValidator.
Reduces complexity score from 12 to 4 per function.
All scenarios remain green.

Scope: post-fix improvement
Health-before: 6.2
Health-after: 8.8
Verified-by: craftsman verify --all
```

The separation is non-negotiable. If the refactoring breaks something, `git revert` the refactoring without affecting the fix. If the fix needs further adjustment, its commit is clean and isolated.

## Code Health–Guided Refactoring

CodeScene's MCP-guided refactoring workflow achieves **2–5× more Code Health improvements** compared to unguided agent refactoring, while maintaining ~95% test pass rate:

1. **Baseline:** Run `code_health_review` on the affected files — get a score and specific issues (complexity, coupling, size, cognitive load)
2. **Plan:** The agent formulates a refactoring plan targeting the specific issues identified — not guessing at improvements
3. **Execute:** Refactor in 3–5 small, reviewable steps
4. **Verify after each step:** Run tests + re-check Code Health
5. **Confirm:** Measurable improvement, no regressions

The key constraint from CodeScene's AGENTS.md guidance: "The AI refactors for maintainability, not passing tests." This is the distinction between a patch and a proper fix: a patch makes the test pass, a proper fix makes the code healthier while keeping tests green.

## What Craftsman Dev Should Adopt

### The Fix Protocol (added to the skill)

```
Bug detected (red scenario, regression, report)
    │
    ├── PHASE 1: DIAGNOSE
    │   ├── Reproduce the bug mechanically
    │   ├── Isolate the root cause (git bisect, binary search, trace)
    │   ├── Formulate a single hypothesis
    │   └── Report diagnosis before fixing
    │       └── If architectural: draft ADR with diagnosis
    │
    ├── PHASE 2: FIX (single commit)
    │   ├── Write a failing test for the root cause
    │   ├── Apply the minimal fix — root cause only
    │   ├── Run full verification — no regressions
    │   └── Commit with structured message (root-cause, ref, verified-by)
    │
    └── PHASE 3: IMPROVE (separate commit, optional but measured)
        ├── Run code health review on touched files
        ├── If health degraded → mandatory refactoring
        ├── Refactor in small, verifiable steps
        ├── Verify after each step
        └── Commit separately (health-before, health-after, verified-by)
```

### Separation of Fix and Refactoring (non-negotiable)

**Fix commit:** changes the behavior (makes the failing test pass). Nothing else.
**Refactoring commit:** changes the structure (improves code health). Nothing else.

Both are independently verifiable. Both are independently revertable. They reference each other in commit messages but are separate git atoms.

This maps to the "Refactoring Runaway" paper's strongest recommendation: "separating refactoring-related transformations from bug-fix generation into independently verifiable stages."

### Code Health as a Fix Gate

At every fix boundary:

```bash
craftsman verify    # all scenarios still green?
craftsman lint      # style clean?
craftsman health    # code health ≥ previous score?
```

If `craftsman health` shows degradation from the fix, the agent enters Phase 3 (Improve) before reporting the fix as complete. The fix doesn't degrade the codebase — it either maintains or improves quality.

### Root-Cause Tests as Regression Insurance

Every fix produces a test that catches the specific root cause, not just the symptom. This test becomes part of the permanent regression suite. It documents *what went wrong* in executable form:

```python
def test_expired_token_rejected():
    """Regression: expired JWT tokens were passing validation.
    Root cause: middleware checked signature but not expiry.
    Fix: added expiry check in TokenValidator.validate()
    Ref: fix commit abc123, ADR-005
    """
    expired_token = create_token(expired=True)
    with pytest.raises(TokenExpiredError):
        validator.validate(expired_token)
```

The docstring is the ADR-in-miniature — explaining the root cause, the fix, and the reference. Any future developer (or agent) reading the test understands both *what* is being tested and *why*.

## Comparison with Other Approaches

| Dimension | Default agent | Superpowers | Systematic Debugging | Craftsman Dev (proposed) |
|---|---|---|---|---|
| Diagnosis | Skip — jump to fix | Agent review (opinion) | 4-phase mandatory | 4-phase mandatory, report-before-fix |
| Fix scope | Whatever makes tests pass | Per-task subagent | Root cause only | Root cause only, minimal commit |
| Refactoring | Tangled with fix | Not addressed | Not addressed | Separate commit, health-gated |
| Quality gate | Tests pass | Tests pass + agent review | Tests pass | Tests + lint + health score |
| Root-cause test | Rarely | Not specified | Mandatory | Mandatory with docstring |
| Revertability | Mixed commits | Mixed commits | Fix only | Fix and refactor independently revertable |

## What NOT to Adopt

**LLM-as-judge for fix quality** — some approaches use a second LLM to evaluate whether a fix is "good enough." This is the same failure mode Craftsman Dev rejects everywhere: an opinion where a measurement belongs. Code health score is a measurement. Test pass rate is a measurement. "This fix looks reasonable" is a vibe.

**Aggressive refactoring during bug fixing** — the temptation to "fix everything while you're in there" leads to tangled commits and compound risk. The research is clear: scope-limited fixes produce better outcomes than opportunistic refactoring mixed with fixes.

**Automated root-cause analysis without reproduction** — some tools attempt to diagnose bugs from stack traces alone, without reproducing them. A root cause you can't reproduce is a hypothesis, not a diagnosis. Reproduction is non-negotiable.

## Conclusion

High-quality bug fixing in agentic development requires enforcing what disciplined human developers already know: diagnose before you fix, fix the root cause not the symptom, separate fixes from refactoring, and leave the codebase healthier than you found it.

The three-phase protocol (Diagnose → Fix → Improve) maps directly to Craftsman Dev's three-actor model:

- **Human** gates the diagnosis (approves the root-cause analysis before the fix proceeds)
- **Agent** implements the minimal fix and the separate refactoring
- **Machine** verifies both mechanically (tests + health score)

The research's strongest finding: **separating fix from refactoring into independent, verifiable commits** prevents the "refactoring runaway" that degrades 21% of agent-generated patches. Each commit does one thing. Each is independently revertable. Each is independently measurable. That's the craftsman's standard applied to bug fixing.
