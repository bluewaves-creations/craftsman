Feature: Todo list
  Scenario: Add an item to the list
    Given an empty todo list
    When I add "buy milk"
    Then the list contains 1 items

  Scenario: Adding one item yields two items
    Given an empty todo list
    When I add "buy milk"
    Then the list contains 2 items

  Scenario: Remove an item from the list
    Given an empty todo list
    When I remove "buy milk"
    Then the list contains 0 items
