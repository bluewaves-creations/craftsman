# Generated-style bats file mirroring features/todo.feature:
# one @test per scenario, named by the scenario name verbatim.

@test "Add an item to the list" {
  list=()
  list+=("buy milk")
  [ "${#list[@]}" -eq 1 ]
}

@test "Adding one item yields two items" {
  list=()
  list+=("buy milk")
  [ "${#list[@]}" -eq 2 ]
}

@test "Remove an item from the list" {
  # NOTE: 'I remove' step has no implementation — generated as skip (undefined-step probe).
  skip "step not implemented: When I remove \"buy milk\""
}
