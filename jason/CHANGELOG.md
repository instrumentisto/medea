Change Log
==========

All user visible changes to this project will be documented in this file. This project uses to [Semantic Versioning 2.0.0].




## [0.1.0] · 2019-08-13
[0.1.0]: https://github.com/instrumentisto/medea/releases/tag/medea-jason-0.1.0

[Milestone](https://github.com/instrumentisto/medea/milestone/1) |
[Roadmap](https://github.com/instrumentisto/medea/issues/8)

### Added

- Setup transport and messaging [#18](https://github.com/instrumentisto/medea/pull/18):
    - External Jason API:
        - `new Jason()`
        - `Jason.join_room()`
        - `Jason.dispose()`
- Use provided ICE servers [#20](https://github.com/instrumentisto/medea/pull/20).
- Signalling [#22](https://github.com/instrumentisto/medea/pull/22):
    - External Jason API:
       - `RoomHandle.on_new_connection` callback 
    - Handle RPC events:
        - `PeerCreated`
        - `SdpAnswerMade`
        - `IceCandidateDiscovered`
        - `PeersRemoved`
    - Emit RPC commands:
        - `MakeSdpOffer`
        - `MakeSdpAnswer`
        - `SetIceCandidate`
- Media management [#22](https://github.com/instrumentisto/medea/pull/22):
    - External Jason API:
        - `MediaStreamHandle.get_media_stream()`
        - `ConnectionHandle.on_remote_stream` callback
        - `Jason.on_local_stream` callback
- Demo application [#38](https://github.com/instrumentisto/medea/pull/38).
- Demo application [Helm] integration [41](https://github.com/instrumentisto/medea/pull/41).




[Semantic Versioning 2.0.0]: https://semver.org
[Helm]: https://helm.sh