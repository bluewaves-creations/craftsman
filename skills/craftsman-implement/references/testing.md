# Emergent Tests

Loaded when the `batch` gear reaches refactor-while-green and it is time to write the tests that live below SPEC.md.

SPEC.md acceptance scenarios are not yours to write or touch — they are the human's contract and `craftsman verify` is their judge. This file governs everything beneath them: the unit, integration, and property tests that emerge from implementation and live alongside the code. They are engineering artifacts, not specification; write them for the value they add, never for volume. Measured result: the number of agent-written tests has no effect on outcomes — only their assertion quality does.

## What earns a unit test

Write a unit test when the code has something a test can catch:

- **Non-trivial logic** — an algorithm, a calculation, a policy decision.
- **Branches** — every conditional path a scenario does not force, especially the sad ones.
- **Transformations** — mapping, parsing, formatting, normalization; assert on the shape and the values.
- **Error paths** — each distinct failure mode raises/returns the documented error, not a generic one.
- **Boundaries** — empty, one, many; zero, negative, max; off-by-one edges of every range you handle.

Example: a `parse_duration("1h30m")` helper earns tests for `"90m"`, `"0s"`, `""`, `"garbage"`, and overflow — five behaviors a single happy-path scenario will never exercise.

## What never earns a unit test

- Getters, setters, constructors that only assign.
- Pure delegation — a function whose body is one call to another tested function.
- Framework boilerplate — routing tables, DI wiring, derived/generated code.
- Behavior already exercised by a SPEC.md scenario with no additional logic of its own.

A test on trivial code is negative value: it pins the implementation, slows the suite, and asserts nothing a plausible bug could break.

## The right-level rule

Decide the level before writing the test:

- Behavior covered by a Gherkin scenario → **do not duplicate it as a unit test**. The scenario is the verdict; a copy below it is maintenance debt.
- Internal complexity the scenario cannot reach (edge cases, error branches, private invariants) → **unit test exactly those paths**.
- Confirm reach with `craftsman verify --impact` — if the scenario already exercises the path, stop.

## Integration tests at real module boundaries

Write an integration test where two modules meet for real — these catch what unit tests structurally cannot:

- **Contracts** — caller and callee agree on the actual interface, run together (endpoint + real database, service + fake-server external API, middleware + handler).
- **Serialization** — what one side writes, the other side reads back, byte-compatible across the boundary. Roundtrip the real wire format, not an in-memory struct.
- **Transactions** — scope, rollback on failure, and isolation behave as the code assumes. Assert the rollback path, not just the commit.

Test against the real boundary technology (in-process database, spawned process, fake HTTP server) — a mock of the boundary tests your assumptions, not the contract.

## Property-based tests — invariant-heavy code only

PBT pays on parsing, encoding, stateful cores, and algebraic logic. It does not pay on glue, I/O orchestration, or CRUD — do not scatter properties for coverage theater. Exactly three patterns earn a property:

**Roundtrip** — encode/decode, serialize/parse, save/load return the original.

```python
@given(st.text())
def test_roundtrip(s): assert decode(encode(s)) == s   # Hypothesis
```

**Invariant** — a truth that holds for every input.

```ts
fc.assert(fc.property(fc.array(fc.integer()), a => isSorted(mySort(a))));  // fast-check
```

```swift
property("sort preserves length") <- forAll { (xs: [Int]) in mySort(xs).count == xs.count }  // SwiftCheck
```

**Oracle** — the fast implementation agrees with an obviously-correct slow one.

```rust
proptest!(|(v in prop::collection::vec(any::<u32>(), 0..100))| {
    prop_assert_eq!(fast_median(&v), naive_median(&v)) });  // proptest
```

Shrunk counterexamples are diagnoses — when a property fails, the minimal input is your failing-first test case.

## Quality rules — write tests that survive mutation

`craftsman mutate` scores changed lines mechanically; these rules are how you pass it on the first run:

- **Every test asserts something a plausible bug would break.** Before keeping a test, name the mutation it kills (flipped comparison, off-by-one, dropped branch, swapped argument). A test that passes against the mutant is a test you delete or strengthen.
- **No print-debugging dressed as a test.** A test that calls the code and prints, logs, or asserts only "did not throw" is a printf, not a verification — the documented default failure mode of agent-written tests. Assert on returned values and observable effects.
- **Test the behavior, not the implementation.** Assert what the caller observes (outputs, state changes, emitted errors), never call order, private fields, or internal structure. A refactor that preserves behavior must leave every test green — that is what refactor-while-green means.
- **One behavior per test, named for the behavior** — `test_rejects_expired_token`, not `test_token_2`. When it fails at a boundary weeks later, the name is the diagnosis.

Coverage is a floor, never a target — executing lines while asserting nothing is the exact gaming pattern the mutation gate exists to catch.
