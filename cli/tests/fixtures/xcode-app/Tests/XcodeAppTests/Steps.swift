// Real step implementations for the xcodebuild round-trip fixture.
// Scenario A and the outline are implemented; Scenario B's step keeps the
// generated stub body — its "step not implemented:" marker is what maps it
// to Undefined through the xcresult bundle.

import Testing

struct SpecSteps {
    var counter = 0
    var limit = 0
    var quantity = 0

    mutating func step_a_seeded_counter() throws { counter = 1 }

    mutating func step_the_counter_is_bumped() throws { counter += 1 }

    mutating func step_the_counter_holds_two() throws {
        #expect(counter == 2, "counter was \(counter)")
    }

    mutating func step_an_unwritten_step() throws {
        #expect(Bool(false), "step not implemented: Given an unwritten step")
    }

    mutating func step_a_limit_of_10() throws { limit = 10 }

    mutating func step_the_quantity_is_set_to(_ quantity: Int) throws {
        self.quantity = quantity
    }

    mutating func step_acceptance_is(_ verdict: String) throws {
        let expected = quantity <= limit ? "accepted" : "rejected"
        #expect(verdict == expected, "quantity \(quantity) with limit \(limit) is \(expected)")
    }
}
