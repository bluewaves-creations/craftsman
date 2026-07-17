// Minimal in-memory domain model backing the generated spec tests.

public enum TodoError: Error, Equatable {
    case notFound(String)
}

public struct TodoList {
    public private(set) var todos: [String] = []
    public private(set) var done: [String] = []

    public init() {}

    public mutating func add(_ title: String) {
        todos.append(title)
    }

    public mutating func complete(_ title: String) throws {
        guard let index = todos.firstIndex(of: title) else {
            throw TodoError.notFound(title)
        }
        todos.remove(at: index)
        done.append(title)
    }
}

public enum CartError: Error, Equatable, CustomStringConvertible {
    case zero
    case negative
    case overLimit

    public var description: String {
        switch self {
        case .zero: "zero"
        case .negative: "negative"
        case .overLimit: "over-limit"
        }
    }
}

public struct Cart {
    public static let maxQuantity = 99
    public private(set) var quantity: Int

    public init(quantity: Int) {
        self.quantity = quantity
    }

    @discardableResult
    public mutating func setQuantity(_ newQuantity: Int) -> Result<Void, CartError> {
        switch newQuantity {
        case 0: return .failure(.zero)
        case ..<0: return .failure(.negative)
        case (Self.maxQuantity + 1)...: return .failure(.overLimit)
        default:
            quantity = newQuantity
            return .success(())
        }
    }
}
