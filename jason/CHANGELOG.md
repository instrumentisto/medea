## [0.1.0] - 2019-08-09
[Milestone](https://github.com/instrumentisto/medea/milestone/1) |
[Roadmap](https://github.com/instrumentisto/medea/issues/8)

Initial release.

#### Implemented

- Setup toolchain (#17)
- Setup transport and messaging (#18)
- Signalling (#22):
  - handle RPC events:
    - `PeerCreated`
    - `SdpAnswerMade`
    - `IceCandidateDiscovered`
    - `PeersRemoved`
  - emit RPC commands:
    - `MakeSdpOffer`
    - `MakeSdpAnswer`
    - `SetIceCandidate`
- Media management (#22):
  - `MediaStream` management
  - `RTCPeerConnection` management
- P2P video calls (#22)


[0.1.0]: https://github.com/instrumentisto/medea/releases/tag/medea-jason-0.1.0
