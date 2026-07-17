// TARGET GENERATED OUTPUT — hand-written stand-in for what `craftsman spec gen`
// must emit from todo.feature. Scenario names become raw-identifier (SE-0451)
// function names so `swift test --filter` and reports match the spec verbatim.

import Testing
@testable import SpecSpike

extension Tag {
    @Tag static var batch1: Self
    @Tag static var batch2: Self
    @Tag static var todo: Self
    @Tag static var cart: Self
}

@Suite("Feature: Todo management")
struct TodoManagementFeature {

    @Test(.tags(.batch1, .todo))
    func `Adding a todo shows it in the list`() async throws {
        var world = TodoWorld()
        world.givenAnEmptyTodoList()
        world.whenIAddATodo("Buy milk")
        world.thenTheListContains("Buy milk")
    }

    @Test(.tags(.batch1, .todo))
    func `Completing a todo moves it to done`() async throws {
        var world = TodoWorld()
        world.givenATodoListContaining("Buy milk")
        try world.whenIComplete("Buy milk")
        world.thenIsInDone("Buy milk")
        world.thenTheListDoesNotContain("Buy milk")
    }

    @Test(.tags(.batch2, .cart), arguments: [
        (quantity: 0, reason: "zero"),
        (quantity: -3, reason: "negative"),
        (quantity: 1000, reason: "over-limit"),
    ])
    func `Rejecting an invalid quantity keeps the cart unchanged`(quantity: Int, reason: String) async throws {
        var world = CartWorld()
        world.givenACartWithQuantity(1)
        world.whenISetTheQuantityTo(quantity)
        world.thenTheUpdateIsRejectedAs(reason)
        world.thenTheCartQuantityIs(1)
    }
}
