Feature: Room closing
  Scenario: Room.on_close fires on Jason.close_room call
    Given room with joined member Alice
    When Alice's room closed by client
    Then Alice's Room.on_close callback fires with `RoomClosed` reason

  Scenario: Room.on_close fires on Jason.dispose call
    Given room with joined member Alice
    When Alice's Jason object disposes
    Then Alice's Room.on_close callback fires with `RoomClosed` reason

  Scenario: Room.on_close fires on Member delete by Control API
    Given room with joined member Alice
    When Control API removes member Alice
    Then Alice's Room.on_close callback fires with `Evicted` reason

  Scenario: Room.on_close fires on Room delete by Control API
    Given room with joined member Alice
    When Control API removes room
    Then Alice's Room.on_close callback fires with `Evicted` reason
