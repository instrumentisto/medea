`medea-client-api-proto` changelog
==================================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## [0.3.0] · 2021-03-19 · To-be-done
[0.3.0]: /../../tree/medea-client-api-proto-0.3.0/proto/client-api

[Diff](/../../compare/medea-client-api-proto-0.2.0...medea-client-api-proto-0.3.0) | [Milestone](/../../milestone/2)

### BC Breaks

- `TracksApplied` event renamed as `PeerUpdated` ([#139]).

[#139]: /../../pull/139




## [0.2.0] · 2021-02-01
[0.2.0]: /../../tree/medea-client-api-proto-0.2.0/proto/client-api

[Diff](/../../compare/medea-client-api-proto-0.1.0...medea-client-api-proto-0.2.0) | [Milestone](/../../milestone/2) | [Roadmap](/../../issues/27)

### BC Breaks

- RPC messages ([#75]):
    - Server messages:
        - `Pong` is now `Ping`.
    - Client messages:
        - `Ping` is now `Pong`.
    - Change `sender` and `receivers` in `Track`'s `Direction` to contain remote `MemberId` instead of `PeerId` ([#124]);
    - Use 32-bit integer types instead of 64-bit ([#115]).

### Added

- `TrackId` and `PeerId` types ([#28]);
- `MemberId` type ([#124]);
- `Incrementable` trait ([#28]);
- `CloseReason` and `CloseDescription` types ([#58]);
- `AddPeerConnectionMetrics` client command with `IceConnectionState` and `PeerConnectionState` metrics ([#71], [#87]);
- `RpcSettings` server message ([#75]);
- `force_relay` field to `PeerCreated` event ([#79]);
- `UpdateTracks` command ([#81]);
- `StatsUpdate` metric into `AddPeerConnectionMetrics` command ([#90]);
- `RTCPeerConnection` stats ([#90]):
    - `RtcCodecStats`;
    - `RtcInboundRtpStreamStats`;
    - `RtcOutboundRtpStreamStats`;
    - `RtcRemoteInboundRtpStreamStats`;
    - `RtcRemoteOutboundRtpStreamStats`;
    - `MediaSourceStats`;
    - `RtpContributingSourceStats`;
    - `RtcPeerConnectionStats`;
    - `DataChannelStats`;
    - `MediaStreamStats`;
    - `TrackStats`;
    - `RtcRtpTransceiverStats`;
    - `SenderStatsKind`;
    - `ReceiverStatsKind`;
    - `RtcTransportStats`;
    - `RtcSctpTransportStats`;
    - `RtcIceCandidatePairStats`;
    - `RtcIceCandidateStats`;
    - `RtcCertificateStats`;
    - `RtcIceServerStats`.
- `Cancelled` state to the `KnownIceCandidatePairState` ([#102]);
- `required` field to `AudioSettings` and `VideoSettings` ([#106], [#155]);
- `TracksApplied` event with `TrackUpdate::Updated` and `TrackUpdate::Added` variants ([#81], [#105]);
- `ConnectionQualityUpdated` event ([#132]);
- `TrackPatchCommand` ([#127]):
    - `enabled` ([#127], [#155]);
    - `muted` ([#156]).
- `TrackPatchEvent` ([#127]):
    - `enabled_individual` ([#127], [#155]);
    - `enabled_general` ([#127], [#155]);
    - `muted` ([#156]).
- `IceRestart` variant to `TrackUpdate` ([#138]);
- `source_kind` field to `VideoSettings` type ([#145]);
- `RoomId` and `Credential` types ([#148]);
- `JoinRoom` and `LeaveRoom` client messages ([#147]);
- `RoomJoined` and `RoomLeft` server messages ([#147]);
- `StateSynchronized` server message ([#167]);
- `SynchronizeMe` client message ([#167]);
- States for the client and server synchronization ([#167]):
    - `Room`;
    - `Peer`;
    - `Sender`;
    - `Receiver`.

[#28]: /../../pull/28
[#58]: /../../pull/58
[#71]: /../../pull/71
[#75]: /../../pull/75
[#79]: /../../pull/79
[#81]: /../../pull/81
[#87]: /../../pull/87
[#90]: /../../pull/90
[#102]: /../../pull/102
[#105]: /../../pull/105
[#106]: /../../pull/106
[#115]: /../../pull/115
[#132]: /../../pull/132
[#127]: /../../pull/127
[#138]: /../../pull/138
[#145]: /../../pull/145
[#147]: /../../pull/147
[#148]: /../../pull/148
[#155]: /../../pull/155
[#156]: /../../pull/156
[#167]: /../../pull/167




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
