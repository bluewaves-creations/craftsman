# Gherkin Authoring

The house Gherkin subset: every scenario you write here is simultaneously a requirement the human approves, a test function the CLI generates, and a filter argument the CLI passes to a runner — write nothing a code generator cannot carry.

## One scenario, one observable behavior

Each scenario asserts exactly one externally observable outcome. If the Then section verifies two unrelated facts, split it. If a When contains two actions, you are testing a workflow, not a behavior — split it. A scenario that can fail for two different reasons is two scenarios.

## Declarative, never imperative

Steps describe behavior, never mechanics. No UI selectors, no button labels, no function names, no HTTP verbs, no table names. The scenario must survive a rewrite of the UI, the API, and the storage layer unchanged — only a change in *behavior* may break it.

```gherkin
# BAD — scripted UI walkthrough, dies on the first redesign
Scenario: Add a todo item
  Given I click the "+" button in the top-right corner
  When I type "Buy milk" into the #new-todo input and press Enter
  Then the <ul class="todo-list"> contains a new <li>

# GOOD — behavior only
Scenario: Adding a todo shows it in the list
  Given an empty todo list
  When the user adds "Buy milk"
  Then the todo list contains exactly "Buy milk"
```

## Concrete values, not abstractions

Use real example values — "Buy milk", 30 minutes, €10.00 — never "some item", "a valid input", "an appropriate error". Concrete values are what make a scenario executable and what make disagreements visible at review time. If a value class matters (boundary, empty, oversized), write one scenario per interesting value or use a Scenario Outline.

## Scenario names are stable identifiers

The scenario name becomes the generated test function name (a Swift Testing raw-identifier function, a bats `@test` name) and the exact string fed to `swift test --filter`, `bats -f`, `pytest -k`, and `cucumber --name`. Treat it like an API name:

- **Unique within the feature.** Duplicate names collide as function names and match ambiguously as filters.
- **No regex-hostile characters.** Letters, digits, spaces, hyphens, commas only. No `( ) [ ] { } . * + ? | ^ $ \ / " '` — the name travels through name-filter regexes verbatim.
- **Present-tense behavior statement.** "Expired session redirects to login", not "Test login redirect" or "Should redirect".
- **Rename = spec change.** A renamed scenario is a removed test plus an added one; only the human renames, and the ledger records it.

## Scenario Outline for variations

When one rule has several value-driven examples, use `Scenario Outline` with an `Examples` table — it generates one parameterized test (`@Test(arguments:)`, `pytest.mark.parametrize`), not copy-pasted scenarios.

```gherkin
Scenario Outline: Rejecting an invalid quantity keeps the cart unchanged
  Given a cart containing 2 items
  When the user sets the quantity to <quantity>
  Then the change is rejected with reason "<reason>"
  And the cart still contains 2 items

  Examples:
    | quantity | reason            |
    | 0        | quantity too low  |
    | -1       | quantity too low  |
    | 1000     | exceeds stock     |
```

## Rule for grouping

Use the `Rule:` keyword to group the scenarios that illustrate one business rule (one blue card from example mapping). Rules give the feature file its table of contents; they generate suite-level grouping where the runner supports it.

## Tags are human-owned and orthogonal

Tags mark orthogonal execution facts: `@slow`, `@ios-only`, `@requires-network`. The human owns them. **Never write batch tags** (`@batch-2` and kin) — batching lives in PLAN.md, and the CLI resolves a batch to scenario names itself. A batch tag would weld a plan (dynamic, agent-owned) into the spec (static, human-owned).

## Background, sparingly

`Background` holds only Given steps shared by *every* scenario in the feature, and at most two or three lines. If a reader must scroll up to understand a scenario, inline the context instead. Never put When or Then steps in a Background.

## Anti-patterns

| Anti-pattern | Smell | Fix |
|---|---|---|
| Incidental detail | "Given a user named Bob with email bob@x.com" when the name is irrelevant | Keep only values the outcome depends on |
| Conjunctive step | "When the user logs in and adds an item and checks out" | One action per When; split the scenario |
| "And I click…" | UI mechanics in steps | Restate as the behavior the click achieves |
| Scripted walkthrough | Ten imperative steps mirroring the UI flow | One Given (state), one When (action), Then (outcome) |
| Scenario chaining | "Given the previous scenario succeeded" | Every scenario builds its own Given; order-independence is what makes filtering work |
| Abstract outcome | "Then an appropriate message is shown" | Name the exact message or the exact effect |
| Implementation leak | "Then `SessionStore.expire()` is called" | Assert the observable effect, not the call |

```gherkin
# BAD — chained, conjunctive, abstract
Scenario: Checkout works
  Given the item from the previous scenario is in the cart
  When the user enters payment details and confirms and waits
  Then everything is processed correctly

# GOOD — self-contained, single action, concrete outcome
Scenario: Confirming payment produces an order confirmation
  Given a cart containing "Widget A" priced at €19.99
  When the user confirms payment with a valid card
  Then an order confirmation is issued for €19.99
```

## Before presenting

Run `craftsman spec lint` and fix every finding. Then present to the human — nothing enters SPEC.md without their approval, and only `craftsman verify` ever declares a scenario passing.
