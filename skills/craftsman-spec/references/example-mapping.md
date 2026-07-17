# Example Mapping

A fifteen-minute structured conversation between you and the human that surfaces rules, examples, and unknowns before any scenario is written — the discovery interview, not a workshop.

## The four cards

Example mapping uses four card types; here they are conversation moves, not index cards:

- **Story** (yellow) — the behavior being specified. One per session.
- **Rule** (blue) — a constraint or business rule the behavior must honor.
- **Example** (green) — one concrete case illustrating one rule, with real values.
- **Question** (red) — an unknown neither of you can resolve now.

Keep a running map in the conversation: the story at the top, each rule with its examples beneath it, questions parked at the bottom.

## How to drive it

1. **State the story back.** One sentence, your words, and get a "yes" before proceeding. A wrong story makes every card below it wrong.
2. **Elicit rules one at a time.** Ask "what must always hold?" and "what is never allowed?". Write each rule as a single declarative sentence. Do not move on while a rule is still fuzzy — sharpen it or convert it to a question.
3. **Demand a concrete example per rule.** For every rule, ask the human to walk one real case: actual values, actual outcome. "A session idle 31 minutes redirects to login" — not "old sessions expire". A rule nobody can exemplify is not understood; park it as a question.
4. **Probe the edges.** For each rule, offer one boundary case yourself ("what about exactly 30 minutes?"). The human's answer becomes an example or a new rule.
5. **Park unknowns immediately.** Anything the human hesitates on becomes a question card. Never resolve a question by guessing, and never let it stall the session — park it and move on.

Keep the rhythm brisk: one rule, one or two examples, next rule. If a rule accumulates five examples, it is probably two rules.

A finished map looks like this:

```
Story: Sessions expire after inactivity

Rule: An idle session expires after 30 minutes
  Example: idle 31 min → any authenticated request redirects to login
  Example: idle 29 min → the request succeeds and the timer resets
Rule: Expiry preserves where the user was going
  Example: idle 31 min, requesting /invoices/42 → after re-login, lands on /invoices/42
Question: Does "remember me" change the 30-minute window?
Question: What happens to an in-flight upload when the session expires?
```

Two rules, each with concrete examples, two honest unknowns — that is a session worth drafting from.

## Stop conditions

Read the map before drafting:

- **A rule without an example is not ready.** Either extract an example now or demote the rule to a question.
- **Too many red questions means not ready to spec.** If questions outnumber rules, or any question blocks the core flow, stop — report the questions to the human and wait. Drafting scenarios over open questions is guessing, and guessing is forbidden.
- **A story that split mid-session** means two sessions. Finish the smaller half first.

## From map to Gherkin

- **Story** → the `Feature:` line.
- **Rule** → a `Rule:` block (or a named scenario group when the rule has one scenario).
- **Example** → exactly one `Scenario`, keeping the example's concrete values; a rule whose examples differ only in values becomes one `Scenario Outline` with an `Examples` table.
- **Question** → never a scenario. List every open question explicitly in the draft you present, under a "Deferred — open questions" heading, each with what is blocked on it. The human sees the full list; nothing silently disappears.

Then write per `references/gherkin-authoring.md`, run `craftsman spec lint`, and present the draft — map, scenarios, and open questions together — for human approval.
