Feature: An example feature
    Scenario: Room.on_new_connection callback fires on interconnection
      Given Member Alice
      And Member Bob
      And Member Bob disabled video
      When Alice joins Room
      And Bob joins Room
      Then Alice's Room.on_new_connection callback fires
      And Bob's Room.on_new_connection callback fires