# Stack: Swift

Loaded when implementing Swift code — Apple platforms and open-source/Linux alike. The conventions file still binds.

Assumed floor: swift-format and SwiftLint run as gates, strict concurrency is on. Nothing below restates what they enforce.

## Concurrency — approachable by default

Build with Approachable Concurrency (`SWIFT_APPROACHABLE_CONCURRENCY=YES`, default MainActor isolation). Write single-threaded code on the main actor until a measurement says otherwise. `nonisolated async` functions run on the caller's actor — most async code needs no isolation annotation at all.

Reach for `@concurrent` only when you have a profiled reason to leave the main actor (heavy parse, image decode, crypto). Never sprinkle it "for performance" — unjustified hopping is the modern spelling of premature optimization, and it reintroduces the sendability friction the default just removed.

```swift
// Bad: reflexive escape — forces Sendable ceremony for a fast operation
@concurrent func formatPrice(_ value: Decimal) async -> String { … }

// Good: stays on the caller's actor; annotate only the measured hot path
func formatPrice(_ value: Decimal) -> String { … }
@concurrent func decodeThumbnails(_ data: [Data]) async -> [Image] { … }  // profiled: 120ms
```

## API Design Guidelines — what the compiler can't see

Name for clarity at the point of use, not at the point of declaration. Omit needless words the type system already states; include the words grammar needs. Mutating/nonmutating pairs follow verb/participle (`sort()`/`sorted()`). Boolean members read as assertions (`isEmpty`, `canUndo`). No Objective-C-flavored prefixes or `get` verbs.

```swift
// Bad: reads as a sentence nowhere; repeats types
func removeObject(object: Element, atIndex index: Int)

// Good: reads correctly at the call site — remove(item, at: 3)
func remove(_ member: Element, at position: Int)
```

## Value types first

Model domain state as `struct` and `enum` with exhaustive switches; reserve `class` for identity, shared mutable state, or framework requirements. Make illegal states unrepresentable with enums carrying associated values rather than optional-field clusters.

```swift
// Bad: three optionals, four invalid combinations
struct Download { var url: URL?; var progress: Double?; var error: Error? }

// Good: each state carries exactly its data
enum Download { case idle; case running(URL, progress: Double); case failed(Error) }
```

## App shape — MV with @Observable

Default architecture: plain SwiftUI views + `@Observable` model objects + a thin service/client layer for I/O. Views hold no business logic — they read model state and forward intents. Inject dependencies without a framework: initializer injection of protocol-typed (or closure-typed) services, so scenarios run against fakes.

TCA is not the default. It is a deliberate per-project opt-in recorded as an ADR; absent that ADR, do not introduce it, and do not build MVVM ceremony (view-model-per-view with pass-through bindings) either.

```swift
// Good: model owns logic, view renders; service injected as a protocol
@Observable final class CheckoutModel {
    private let payments: any PaymentService   // fake in specs, live in app
    var cart: Cart
    func placeOrder() async throws { try await payments.charge(cart.total) }
}
```

## Errors

Throw typed errors (`enum` per subsystem conforming to `Error`, with associated values carrying context). Reserve `fatalError`/force-unwrap for provably impossible states, each with a comment stating the invariant. Never discard errors into `try?` on paths a scenario can observe.

## Apple projects

- If Apple's exported Xcode skills are installed (probe: skills present in the agent's skills directory; export with `xcrun mcpbridge run-agent skills export --output-dir ~/.agents/skills --replace-existing`), **defer to them** for SwiftUI and testing idiom — `swiftui-specialist`, `swiftui-whats-new-27`, `modernize-tests` are Apple-maintained and outrank this file on those topics. Do not restate or contradict them.
- The verification surface is headless: `xcodebuild test` / `swift test` exit codes, `xcresulttool get test-results` JSON as evidence. Never depend on the IDE — `mcpbridge`, `device-interaction`, and anything requiring a running Xcode window are opportunistic extras, not gates.
- Tests are Swift Testing (`@Test`, `#expect`, parameterized `arguments:`), not new XCTest. UI automation remains XCTest-side; `performAccessibilityAudit()` belongs in every UI test target.
- On Linux/SwiftPM the same rules apply minus the Apple sections; `swift test --parallel` is the whole verification surface.
