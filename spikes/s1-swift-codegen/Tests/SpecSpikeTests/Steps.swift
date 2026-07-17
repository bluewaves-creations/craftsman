// Step functions shared by generated scenarios. One tiny function per Gherkin step.

import Testing
@testable import SpecSpike

struct TodoWorld {
    var list = TodoList()

    // Given
    mutating func givenAnEmptyTodoList() {
        list = TodoList()
    }

    mutating func givenATodoListContaining(_ title: String) {
        list = TodoList()
        list.add(title)
    }

    // When
    mutating func whenIAddATodo(_ title: String) {
        list.add(title)
    }

    mutating func whenIComplete(_ title: String) throws {
        try list.complete(title)
    }

    // Then
    func thenTheListContains(_ title: String) {
        #expect(list.todos.contains(title), "expected list to contain \(title)")
    }

    func thenTheListDoesNotContain(_ title: String) {
        #expect(!list.todos.contains(title), "expected list not to contain \(title)")
    }

    func thenIsInDone(_ title: String) {
        #expect(list.done.contains(title), "expected done to contain \(title)")
    }
}

struct CartWorld {
    var cart = Cart(quantity: 1)
    var lastResult: Result<Void, CartError>?

    // Given
    mutating func givenACartWithQuantity(_ quantity: Int) {
        cart = Cart(quantity: quantity)
    }

    // When
    mutating func whenISetTheQuantityTo(_ quantity: Int) {
        lastResult = cart.setQuantity(quantity)
    }

    // Then
    func thenTheUpdateIsRejectedAs(_ reason: String) {
        guard case .failure(let error)? = lastResult else {
            Issue.record("expected a rejected update, got \(String(describing: lastResult))")
            return
        }
        #expect(error.description == reason)
    }

    func thenTheCartQuantityIs(_ quantity: Int) {
        #expect(cart.quantity == quantity)
    }
}
