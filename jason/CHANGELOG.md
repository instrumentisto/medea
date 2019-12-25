`medea-jason` changelog
=======================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## TBD [0.2.0] · 2019-??-??
[0.2.0]: /../../tree/medea-jason-0.2.0/jason

[Milestone](/../../milestone/2) | [Roadmap](/../../issues/27)

### BC Breaks

- Library API:
    - Expose `on_local_stream` callback in `Room` instead of `Jason` ([#54]);
    - Remove error argument from `on_local_stream` callback ([#54]);
    - Room initialization ([#46]):
        - Remove `Jason.join_room()`.
        
- Transport and messaging:
    - Respond with `Pong`s to server `Ping`s instead of sending `Ping`s ([#75]).

### Added

- Media management:
    - Library API:
        - Mute/unmute local video/audio ([#40](/../../pull/40)):
            - `Room.mute_audio()`;
            - `Room.unmute_audio()`;
            - `Room.mute_video()`;
            - `Room.unmute_video()`.
        - `InputDeviceInfo` class obtainable via `MediaManager.enumerate_devices()` ([#46]);
        - `MediaManager` class obtainable via `Jason.media_manager()` ([#46]):
            - `MediaManager.enumerate_devices()`;
            - `MediaManager.init_local_stream()`.
        - `MediaStreamConstraints`, `AudioTrackConstraints`, `VideoTrackConstraints` classes ([#46]);
        - Room initialization ([#46]):
            - `Jason.init_room()`;
            - `Room.join()`;
        - Ability to inject local video/audio stream into `Room` via `Room.inject_local_stream()` ([#54]);
        - `Room.on_failed_local_stream` callback ([#54]);
        - `Room.on_close` callback for WebSocket close initiated by server ([#55]);
        - `Room.on_connection_loss` callback with which JS side can start Jason reconnection on connection loss ([#75]).
- RPC messaging:
    - Cleanup Jason state on normal (`code = 1000`) WebSocket close ([#55]);
    - `RpcClient` and `RpcTransport` reconnection ([#75]).
- Signalling:
    - Emitting of RPC commands:
        - `AddPeerConnectionMetrics` with `IceConnectionState` ([#71](/../../pull/71));
- Error handling:
    - Library API:
        - `JasonError` as library error with trace information and underlying JS error if it is the cause ([#55])

### Fixed

- Signalling:
    - Skipped `IceCandidate`s received before receiving remote SDP ([#50](/../../pull/50)).

[#46]: /../../pull/46
[#54]: /../../pull/54
[#55]: /../../pull/55
[#75]: /../../pull/75




## [0.1.0] · 2019-08-21
[0.1.0]: /../../tree/medea-jason-0.1.0/jason

[Milestone](/../../milestone/1) | [Roadmap](/../../issues/8)

### Added

- Transport and messaging ([#18](/../../pull/18)):
    - Library API:
        - `new Jason()`;
        - `Jason.join_room()`;
        - `Jason.dispose()`.
    - RPC transport and heartbeat.
- Ability to use ICE servers provided by server ([#20](/../../pull/20)).
- Signalling ([#22](/../../pull/22)):
    - Library API:
       - `RoomHandle.on_new_connection` callback.
    - Handling of RPC events:
        - `PeerCreated`;
        - `SdpAnswerMade`;
        - `IceCandidateDiscovered`;
        - `PeersRemoved`.
    - Emitting of RPC commands:
        - `MakeSdpOffer`;
        - `MakeSdpAnswer`;
        - `SetIceCandidate`.
- Media management ([#22](/../../pull/22)):
    - Library API:
        - `MediaStreamHandle.get_media_stream()`;
        - `ConnectionHandle.on_remote_stream` callback;
        - `Jason.on_local_stream` callback.





[Semantic Versioning 2.0.0]: https://semver.org
