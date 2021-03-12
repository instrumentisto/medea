Feature: `OnJoin` callback of Control API

  Scenario: `OnJoin` fires when member joins
    Given room with member Alice
    When Alice joins the room
    Then Control API sends `OnJoin` callback for member Alice
