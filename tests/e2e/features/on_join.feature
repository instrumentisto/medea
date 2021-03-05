Feature: OnJoin callback of Control API

  Scenario: OnJoin fires when Member joins
    Given room with member Alice
    When Alice joins the room
    Then Control API sends OnJoin callback for member Alice
