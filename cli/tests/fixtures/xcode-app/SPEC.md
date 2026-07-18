Feature: Xcode round trip

  Scenario: Scenario A passes
    Given a seeded counter
    When the counter is bumped
    Then the counter holds two

  Scenario: Scenario B stays undefined
    Given an unwritten step

  Scenario Outline: Quantities within range are accepted
    Given a limit of 10
    When the quantity is set to <quantity>
    Then acceptance is "<verdict>"

    Examples:
      | quantity | verdict  |
      | 3        | accepted |
      | 12       | rejected |
