Feature: Room closing
  Scenario: Room.on_close fires on Jason.close_room call
    Given joined Member `Alice`
    When `Alice`'s Room closed by client
    Then `Alice`'s Room.on_close callback fires with `RoomClosed` reason

  Scenario: Room.on_close fires on Jason.dispose call
    Given joined Member `Alice`
    When `Alice`'s Jason object disposes
    Then `Alice`'s Room.on_close callback fires with `RoomClosed` reason

  Scenario: Room.on_close fires on Member delete by Control API
    Given joined Member `Alice`
    When Control API removes Member `Alice`
    Then `Alice`'s Room.on_close callback fires with `Evicted` reason

  Scenario: Room.on_close fires on Room delete by Control API
    Given joined Member `Alice`
    When Control API removes Room
    Then `Alice`'s Room.on_close callback fires with `Evicted` reason
