`medea-client-api-proto` changelog
==================================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## TBD [0.2.0] · 2019-??-??
[0.2.0]: /../../tree/medea-client-api-proto-0.2.0/proto/client-api

### BC Breaks

- RPC messages ([#75]):
    - Server messages:
        - `Pong` is now `Ping`.
    - Client messages:
        - `Ping` is now `Pong`.

### Added

- `TrackId` and `PeerId` types ([#28]);
- `Incrementable` trait ([#28]);
- `CloseReason` and `CloseDescription` types ([#58]);
- `AddPeerConnectionMetrics` client command with `IceConnectionState` and `PeerConnectionState` metrics ([#71], [#87]);
- `RpcSettings` server message ([#75]);
- `force_relay` field to `PeerCreated` event ([#79]);
- `UpdateTracks` command and `TracksUpdated` event ([#81]);
- `StatsUpdate` metric into `AddPeerConnectionMetrics` command ([#90]).
- `RTCPeerConnection` stats ([#90]):
    - `RtcCodecStats`;
    - `RtcInboundRtpStreamStats`;
    - `RtcOutboundRtpStreamStats`;
    - `RemoteInboundRtpStreamStat`;
    - `RemoteOutboundRtpStreamStat`;
    - `MediaSourceStat`;
    - `RtpContributingSourceStat`;
    - `RtcPeerConnectionStat`;
    - `DataChannelStat`;
    - `MediaStreamStat`;
    - `TrackStat`;
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
- `TracksAdded` event ([#105]).

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
