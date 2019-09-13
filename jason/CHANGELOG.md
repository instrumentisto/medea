`medea-jason` changelog
=======================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## TBD [0.2.0] · 2019-??-??
[0.2.0]: /../../tree/medea-jason-0.2.0/jason

### Added

- Media management:
    - Library API ([#40](/../../pull/40)):
        - `Room.mute_audio()`;
        - `Room.unmute_audio()`;
        - `Room.mute_video()`;
        - `Room.unmute_video()`.




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
