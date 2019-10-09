`medea-client-api-proto` changelog
==================================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## TBD [0.1.1] · 2019-??-??
[0.1.1]: /../../tree/medea-client-api-proto-0.1.1/proto/client-api

### Added

- `TrackId` and `PeerId` types ([#28]);
- `Incrementable` trait ([#28]);
- `RpcConnectionCloseReason` and `CloseDescription` types ([#58]);
- `RpcConnectionCloseReason` variants:
    - `Finished` ([#58]);
    - `NewConnection` ([#58]);
    - `Idle` ([#58]);
    - `ConnectionRejected` ([#58]);
    - `ServerError` ([#58]).

[#28]: /../../pull/28
[#58]: /../../pull/58




## [0.1.0] · 2019-08-21
[0.1.0]: /../../tree/medea-client-api-proto-0.1.0/proto/client-api

[Milestone](/../../milestone/1) | [Roadmap](/../../issues/8)

### Added

- RPC messages ([#16](/../../pull/16)):
    - Server messages:
        - `Pong`;
        - `Event`.
    - Client messages:
        - `Ping`;
        - `Command`.
    - Client commands:
        - `MakeSdpOffer`;
        - `MakeSdpAnswer`;
        - `SetIceCandidate`.
    - Server events:
        - `PeerCreated`;
        - `SdpAnswerMade`;
        - `IceCandidateDiscovered`;
        - `PeersRemoved`.





[Semantic Versioning 2.0.0]: https://semver.org
