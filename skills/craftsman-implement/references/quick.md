# The Light Path (quick)

Loaded for a small scoped change with no new externally visible behavior: a rename, a config value, a copy tweak, an internal cleanup, a version bump. The whole methodology compressed to what still matters: grounding, gates, ledger.

## Qualify first

`quick` applies only when ALL of these hold:

- No new externally visible behavior (nothing a SPEC.md scenario would describe).
- Not a bug (something that worked now wrong → craftsman-fix, always).
- Scope you can state in one sentence before starting.

If any fails — or stops holding mid-flight — stop and route per the conventions' scale routing. The moment a "quick change" needs a design decision, it wasn't one.

## The loop

1. **Ground** if any API is involved: `craftsman docs` per the Documentation Sources table. Small change ≠ permission to guess a signature.
2. **Impact check**: `craftsman verify --impact` — know which scenarios this can touch before you touch anything. If the impact set is surprisingly large, that is the routing signal you missed in qualification.
3. **Make the change.** Stack idiom still applies (stack file). Production grade still applies — quick is a scope statement, not a quality discount.
4. **Verify and gate**: `craftsman verify --impact`, then `craftsman check-all --changed`. Bounded fixes as always; recovery budget applies.
5. **Commit**: `craftsman commit` — usually type `chore`, `refactor`, or `docs`; body says what and why in two lines; `Learned:` only if there genuinely is one.

Total ceremony: zero artifacts touched, two commands more than a bare edit. That is the entire point — the light path is light because the gates are cheap, not because the standard dropped.

## Never

- Never let `quick` grow behavior — new behavior means craftsman-spec first, even if the code is already written (the code becomes the first draft, not the decision).
- Never skip `check-all --changed` because the change "obviously can't break anything".
- Never chain multiple unrelated quick changes into one commit.
