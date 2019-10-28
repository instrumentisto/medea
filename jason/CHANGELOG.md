`medea-jason` changelog
=======================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## TBD [0.2.0] · 2019-??-??
[0.2.0]: /../../tree/medea-jason-0.2.0/jason

### BC Breaks

- Library API:
    - Room initialization ([#46]):
        - Remove `Jason.join_room()`.

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
            - `Room.join()`.
- Signalling:
    - Cleanup Jason state on normal WebSocket close ([#55](/../../pull/55)).

### Fixed

- Signalling:
    - Skipped `IceCandidate`s received before receiving remote SDP ([#50](/../../pull/50)).

[#46]: /../../pull/46




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
