Feature: Todo list

  Scenario: Add an item to the list
    Given an empty todo list
    When I add "buy milk"
    Then the list contains 1 items

  @slow
  Scenario: Remove an item from the list
    Given an empty todo list
    Then the list contains 0 items

  Scenario Outline: Checking quantities in bulk
    Given an empty todo list
    Then the list contains <count> items

    Examples:
      | count |
      | 0     |
      | 0     |
