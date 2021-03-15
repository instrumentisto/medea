Feature: Apply method of Control API
#  Scenario: Remove Member by apply
#    Given room with joined member Alice and Bob
#    When Control API removes Alice by apply
#    Then Bob's connection with Alice closes

  Scenario: Interconnect Members by apply
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Control API interconnects Alice and Bob by apply
    Then Alice receives connection with Bob
    And Bob receives connection with Alice

#  Scenario: OnJoin callback fires on interconnection by applying
#    Given room with joined member Alice and Bob with no WebRTC endpoints
#    When Control API interconnects Alice and Bob by apply
#    Then Control API sends `OnJoin` callback for member Alice

#  Scenario: `Room.on_close()` fires when room is removed by apply
#    Given room with joined member Alice
#    When Control API removes Alice by apply
#    Then Alice's `on_close` room's callback fires with `Evicted` reason
