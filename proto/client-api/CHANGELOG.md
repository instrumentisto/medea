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

- Commands:
    - `AddPeerConnectionMetrics` with `IceConnectionState`, `PeerConnectionState` and `StatsUpdate` metrics ([#71], [#87], [#90]);
    - `UpdateTracks` ([#58]);
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
- Events: 
    - `RpcSettings` ([#75]);
    - `force_relay` field to `PeerCreated` event ([#79]);
    - `TracksUpdated` ([#81]);
    - `SdpOfferMade` ([#89]);
    - `RenegotiationStarted` ([#89]);
- Misc:
    - `TrackId` and `PeerId` types ([#28]);
    - `Incrementable` trait ([#28]);
    - `CloseReason` and `CloseDescription` types ([#58]).

[#28]: /../../pull/28
[#58]: /../../pull/58
[#71]: /../../pull/71
[#75]: /../../pull/75
[#79]: /../../pull/79
[#81]: /../../pull/81
[#87]: /../../pull/87
[#89]: /../../pull/89
[#90]: /../../pull/90




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
