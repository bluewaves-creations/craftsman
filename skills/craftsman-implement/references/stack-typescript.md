# Stack: TypeScript

Loaded when implementing TypeScript code. The conventions file still binds.

Assumed floor: `tsc` with the full strict flag set (including `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`) and Biome for lint + format. Nothing below restates what they enforce. New code is ESM-only — `"type": "module"`, no CJS output, no dual-package builds; that minefield is a documented dead end.

## Parse, don't validate

Untrusted data — HTTP bodies, env vars, file contents, LLM output — crosses the boundary through a Zod v4 schema exactly once, producing a typed value the rest of the program trusts. Infer the type from the schema (`z.infer`), never hand-write a twin interface. Inside the boundary there is no re-checking, no defensive `typeof`, no optional-everything types.

```typescript
// Bad: a type assertion is a promise nobody checked
const order = JSON.parse(body) as Order;

// Good: one parse at the edge; the type is born from the schema
const OrderSchema = z.object({
  id: z.uuid(),
  lines: z.array(LineSchema).min(1),
});
type Order = z.infer<typeof OrderSchema>;
const order = OrderSchema.parse(JSON.parse(body)); // throws structured ZodError
```

## Discriminated unions over class hierarchies

Model states and variants as tagged unions with exhaustive `switch` (and a `never` default arm), not as class trees with instanceof checks or optional-field clusters. Unions make illegal states unrepresentable and let inference narrow for free.

```typescript
// Bad: four fields, most combinations invalid
interface Fetch { loading?: boolean; data?: User; error?: Error }

// Good: each state carries exactly its data; switch is exhaustive
type Fetch =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "done"; data: User }
  | { status: "failed"; error: Error };
```

## The one-token escapes — constructive alternatives

The gates ban `as any`, non-null `!`, and `@ts-ignore`. Each has a real fix; reaching for the escape means the model is wrong, not the checker.

- **Instead of `as any`**: the value's type is genuinely unknown — so treat it as `unknown` and parse it (Zod schema or a type-guard function). If you're fighting a library's bad types, wrap the library at one adapter module and give the wrapper honest types; the escape stays quarantined behind a typed surface.
- **Instead of `!`**: prove the presence. Narrow with an early return (`if (!user) return …`), use `??` when a default is correct, or restructure so the value is non-optional at construction (parse it into a required field). If it truly cannot be null, encode why: throw a descriptive error at the check site — that's a boundary assertion the runtime enforces, unlike `!` which erases it.
- **Instead of `@ts-ignore`**: fix the type. When suppressing is unavoidable (upstream bug, tracked), `@ts-expect-error` with a comment and an issue link is the only acceptable form — it self-destructs when the underlying error is fixed.

```typescript
// Bad
sendReceipt(order.customer!.email);

// Good: narrowing is the proof the compiler wanted
const customer = order.customer;
if (!customer) throw new MissingCustomerError(order.id);
sendReceipt(customer.email);
```

## Errors and results

Throw `Error` subclasses with structured fields (`cause`, domain data as properties) at boundaries; never throw strings. Where a failure is an expected outcome the caller must handle (validation verdicts, lookup misses), return a discriminated result union (`{ ok: true; value } | { ok: false; error }`) so the type system forces handling — but don't Result-ify code whose only sane response to failure is propagation.

## Not house style

effect-ts is not the default. It is viral by design — once one function returns `Effect`, it owns the control flow of everything that calls it. Adopting it is a per-project decision recorded as an ADR; absent that ADR, do not introduce it, and do not hand-roll a monadic effect layer as a substitute. The same ADR bar applies to any framework that wants to own program structure (fp-ts, dependency-injection containers).
