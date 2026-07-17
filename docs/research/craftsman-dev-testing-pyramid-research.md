# The Testing Pyramid: Research & Architecture

> How Gherkin acceptance tests, unit tests, integration tests, and property-based tests relate in Craftsman Dev — evaluated against the 2026 landscape of agent-era testing strategy.

---

## The Paradigm Shift

The 2026 consensus on TDD with agents has shifted: "The atomic unit so far with TDD was the unit test driving a specific condition. With the advent of sophisticated agents, this atomic unit has become an integration/acceptance test driving an acceptance criterion in one iteration."

This validates Craftsman Dev's architecture: SPEC.md (Gherkin scenarios) defines "done" at the acceptance level. But where do unit and integration tests fit?

## The Craftsman Testing Pyramid

```
        ╱ Acceptance (Gherkin)  ╲  ← SPEC.md, few, high-confidence
       ╱─────────────────────────╲
      ╱ Property-based (invariants)╲  ← complement to acceptance
     ╱─────────────────────────────╲
    ╱    Integration tests           ╲  ← cross-module boundaries
   ╱─────────────────────────────────╲
  ╱         Unit tests                 ╲  ← emergent from implementation
 ╱─────────────────────────────────────╲
```

### Layer 1: Acceptance Tests (Human-Specified, in SPEC.md)

Gherkin scenarios. Written by the expert agent from official docs, approved by the human. These define *what done looks like*. They don't change during implementation. They're the top of the pyramid: few, slow, high-confidence, and high-value.

These are the tests that *matter* for the batch boundary: `craftsman verify` runs these, and their pass/fail state determines whether a batch is complete.

### Layer 2: Property-Based Tests (Agent-Written, Invariant Coverage)

Hypothesis, fast-check, proptest, SwiftCheck. These express invariants that must hold for *all* inputs, not just the examples in Gherkin scenarios. The agent writes these during implementation when it identifies properties that should hold universally.

Properties complement Gherkin: the scenario says "adding an item adds it to the list." The property says "for any list and any valid item, adding it increases the list length by exactly one."

These run alongside Gherkin in `craftsman verify` but are secondary to acceptance tests for batch pass/fail decisions.

### Layer 3: Integration Tests (Agent-Written, Cross-Module)

Tests that verify modules work together correctly. The agent writes these when implementation crosses module boundaries: API endpoint + database, service + external API, middleware + handler.

These are especially important for catching issues that unit tests miss: contract violations between modules, serialization/deserialization mismatches, transaction scope errors.

Integration tests run as part of the full test suite but are not individually tracked in SPEC.md.

### Layer 4: Unit Tests (Agent-Written, Emergent)

Unit tests emerge from implementation. When the agent writes a function with non-trivial logic (conditional branches, data transformation, error handling), it writes unit tests for that function. But not for trivial code — Kent Beck's rule applies: don't test getters, setters, or simple delegation.

Unit tests are the fastest feedback loop: they catch logic errors before the agent even runs the acceptance tests. They're the base of the pyramid: many, fast, cheap.

## The Relationship Between Layers

The key design decision: **SPEC.md only contains acceptance-level scenarios.** Unit and integration tests are *implementation artifacts*, not specification artifacts. They emerge from the agent's implementation work and live alongside the code.

```
SPEC.md (human-approved)
  └── Acceptance tests: "what done looks like"
       └── drives implementation, which produces:
            ├── Unit tests (per-function logic)
            ├── Integration tests (cross-module contracts)
            └── Property tests (universal invariants)
```

The human specifies *what*. The agent implements *how* and writes the tests that verify *how*. The machine runs all levels and reports pass/fail.

This maps directly to the three-actor model: human specifies (acceptance), agent implements (unit + integration + property), machine verifies (all levels).

## What the Agent Should Test (and Shouldn't)

**Always test:** Non-trivial business logic, error handling paths, data transformations, boundary conditions, cross-module contracts, security-relevant code.

**Never test:** Getters/setters, trivial delegation, framework boilerplate, code that's already tested by acceptance scenarios with no additional logic.

**Test at the right level:** If a behavior is covered by a Gherkin scenario, don't duplicate it as a unit test. If a function has internal complexity that the Gherkin scenario doesn't exercise (edge cases, error paths), add unit tests for those paths.

## Conclusion

Craftsman Dev's testing pyramid is acceptance-driven at the top (human-specified Gherkin scenarios), with unit, integration, and property tests emerging from implementation. SPEC.md is the contract. Everything below is the agent's engineering judgment, verified mechanically by `craftsman verify --all` which runs every level of the pyramid in a single pass.
