Change Log
==========

All user visible changes to this project will be documented in this file. This project uses to [Semantic Versioning 2.0.0].




## [0.1.0] · 2019-08-13
[0.1.0]: https://github.com/instrumentisto/medea/releases/tag/medea-client-api-proto-0.1.0

[Milestone](https://github.com/instrumentisto/medea/milestone/1) |
[Roadmap](https://github.com/instrumentisto/medea/issues/8)

### Added

- Client API RPC messages [#16](https://github.com/instrumentisto/medea/pull/16): 
    - Server messages:
        - `Pong`
        - `Event`
    - Client messages:
        - `Ping`
        - `Command`
    - Client commands:
        - `MakeSdpOffer`
        - `MakeSdpAnswer`
        - `SetIceCandidate`
    - Server events:
        - `PeerCreated`
        - `SdpAnswerMade`
        - `IceCandidateDiscovered`
        - `PeersRemoved`




[Semantic Versioning 2.0.0]: https://semver.org