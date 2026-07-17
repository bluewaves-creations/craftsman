Feature: Todo management

  @batch1 @todo
  Scenario: Adding a todo shows it in the list
    Given an empty todo list
    When I add a todo "Buy milk"
    Then the list contains "Buy milk"

  @batch1 @todo
  Scenario: Completing a todo moves it to done
    Given a todo list containing "Buy milk"
    When I complete "Buy milk"
    Then "Buy milk" is in done
    And the list does not contain "Buy milk"

  @batch2 @cart
  Scenario Outline: Rejecting an invalid quantity keeps the cart unchanged
    Given a cart with quantity 1
    When I set the quantity to <quantity>
    Then the update is rejected as "<reason>"
    And the cart quantity is 1

    Examples:
      | quantity | reason     |
      | 0        | zero       |
      | -3       | negative   |
      | 1000     | over-limit |
