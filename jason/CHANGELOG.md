`medea-jason` changelog
=======================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## TBD [0.2.0] · 2019-??-??
[0.2.0]: /../../tree/medea-jason-0.2.0/jason

[Milestone](/../../milestone/2) | [Roadmap](/../../issues/27)

### BC Breaks

- Library API:
    - Replace `MediaStreamHandle` with `LocalMediaStream` and `RemoteMediaStream` ([#97]);
    - Expose `on_local_stream` callback in `Room` instead of `Jason` ([#54]);
    - Remove error argument from `on_local_stream` callback ([#54]);
    - Room initialization ([#46]):
        - Remove `Jason.join_room()`.
- Transport and messaging:
    - Reverse `ping`/`pong` mechanism: expect `Ping`s from server and answer with `Pong`s ([#75]).

### Added

- Media management:
    - Library API:
        - Mute/unmute local video/audio ([#40], [#81], [#97]):
            - `Room.mute_audio()`;
            - `Room.unmute_audio()`;
            - `Room.mute_video()`;
            - `Room.unmute_video()`.
        - `InputDeviceInfo` class obtainable via `MediaManager.enumerate_devices()` ([#46]);
        - `MediaManager` class obtainable via `Jason.media_manager()` ([#46]):
            - `MediaManager.enumerate_devices()`;
            - `MediaManager.init_local_stream()`.
        - Local media stream constraints:
            - `MediaStreamSettings`, `AudioTrackConstraints` classes ([#46], [#97]);
            - `DeviceVideoTrackConstraints`, `DisplayVideoTrackConstraints` classes ([#78]).
        - Room initialization ([#46]):
            - `Jason.init_room()`;
            - `Room.join()`;
        - Ability to configure local media stream used by `Room` via `Room.set_local_media_settings()` ([#54], [#97]);
        - `Room.on_failed_local_stream` callback ([#54]);
        - `Room.on_close` callback for WebSocket close initiated by server ([#55]);
        - `RemoteMediaStream.on_track_enabled` and `RemoteMediaStream.on_track_disabled` callbacks being called when `MediaTrack` is enabled or disabled ([#123]);
        - `RemoteMediaStream.on_track_added` callback being called when new receiver `MediaTrack` is added ([#123]);
        - `RemoteMediaStream.has_active_audio` and `RemoteMediaStream.has_active_video` methods returning current state of the receivers ([#123]).
    - Optional tracks support ([#106]);
    - `RtcIceTransportPolicy` configuration ([#79]).
- Room management:
    - Library API:
        - `Room.on_connection_loss` callback that JS side can start Jason reconnection on connection loss with ([#75]);
        - `Room.on_close` callback for WebSocket close initiated by server ([#55]);
        - `ConnectionHandle.on_close` callback ([#120]);
        - `ConnectionHandle.get_remote_member_id` method ([#124]);
        - `ConnectionHandle.on_quality_score_update` callback for quality score updates received from server ([#132]).
- RPC messaging:
    - Cleanup Jason state on normal (`code = 1000`) WebSocket close ([#55]);
    - `RpcClient` and `RpcTransport` reconnection ([#75]).
- Signalling:
    - Emitting of RPC commands:
        - `AddPeerConnectionMetrics` with `IceConnectionState` and `PeerConnectionState` ([#71], [#87]);
        - `ApplyTracks` for muting/unmuting ([#81]);
        - `AddPeerConnectionStats` with `RtcStats` ([#90]);
    - Handling of RPC events:
        - `TracksApplied` ([#105]);
        - `ConnectionQualityUpdated` ([#132]).
- Error handling:
    - Library API:
        - `JasonError` as library error with trace information and underlying JS error if it is the cause ([#55])

### Fixed

- Signalling:
    - Skipped `IceCandidate`s received before receiving remote SDP ([#50]).

[#40]: /../../pull/40
[#46]: /../../pull/46
[#50]: /../../pull/59
[#54]: /../../pull/54
[#55]: /../../pull/55
[#71]: /../../pull/71
[#75]: /../../pull/75
[#78]: /../../pull/78
[#79]: /../../pull/79
[#81]: /../../pull/81
[#87]: /../../pull/87
[#90]: /../../pull/90
[#97]: /../../pull/97
[#105]: /../../pull/105
[#106]: /../../pull/106
[#120]: /../../pull/120
[#123]: /../../pull/123
[#124]: /../../pull/124
[#132]: /../../pull/132




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
