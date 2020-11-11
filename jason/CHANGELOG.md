`medea-jason` changelog
=======================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## TBD [0.2.0] · 2019-??-??
[0.2.0]: /../../tree/medea-jason-0.2.0/jason

[Milestone](/../../milestone/2) | [Roadmap](/../../issues/27)

### BC Breaks

- Library API:
    - Remove `MediaStreamHandle` ([#143]);
    - Expose `on_local_track` callback in `Room` instead of `Jason` ([#54], [#143]);
    - Replace `on_local_stream` callback with `on_local_track` ([#143]);
    - Room initialization ([#46]):
        - Remove `Jason.join_room()`.
- Transport and messaging:
    - Reverse `ping`/`pong` mechanism: expect `Ping`s from server and answer with `Pong`s ([#75]).

### Added

- Media management:
    - Library API:
        - Disable/Enable local video/audio ([#40], [#81], [#97], [#144], [#155]):
            - `Room.disable_audio()`;
            - `Room.enable_audio()`;
            - `Room.disable_video()`;
            - `Room.enable_video()`.
        - `InputDeviceInfo` class obtainable via `MediaManager.enumerate_devices()` ([#46]);
        - `MediaManager` class obtainable via `Jason.media_manager()` ([#46]):
            - `MediaManager.enumerate_devices()`;
            - `MediaManager.init_local_tracks()` ([#46], [#143]).
        - Local media stream constraints:
            - `MediaStreamSettings`, `AudioTrackConstraints` classes ([#46], [#97]);
            - `DeviceVideoTrackConstraints`, `DisplayVideoTrackConstraints` classes ([#78]);
            - `DeviceVideoTrackConstraints.ideal_facing_mode` and `DeviceVideoTrackConstraints.exact_facing_mode` functions ([#137]);
            - `FacingMode` enum ([#137]).
        - `MediaKind` enum that provides `MediaTrack` and `InputDeviceInfo` kind ([#146]);
        - `MediaSourceKind` enum that provides `MediaTrack` media source kind (`Device` or `Display`) ([#146]);
        - Room management:
            - `Jason.init_room()` ([#46]);
            - `Room.join()` ([#46]);
            - `Jason.close_room()` ([#147]).
        - Ability to configure local media stream used by `Room` via `Room.set_local_media_settings()` ([#54], [#97], [#145]);
        - `Room.on_failed_local_media` callback ([#54], [#143]);
        - `Room.on_close` callback for WebSocket close initiated by server ([#55]);
        - `MediaTrack.on_enabled` and `MediaTrack.on_disabled` callbacks being called when `MediaTrack` is enabled or disabled ([#123], [#143]);
        - `ConnectionHandle.on_remote_track_added` callback being called when new receiver `MediaTrack` is added ([#123], [#143]);
        - Enabling/disabling remote video/audio ([#127], [#155]):
            - `Room.disable_remote_audio`;
            - `Room.enable_remote_audio`;
            - `Room.disable_remote_video`;
            - `Room.enable_remote_video`.
        - `MediaTrack.media_source_kind` function ([#145], [#146]).
    - Optional tracks support ([#106]);
    - Simultaneous device and display video tracks publishing and receiving ([#144]);
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
        - `ApplyTracks` for enabling/disabling ([#81], [#155]);
        - `AddPeerConnectionStats` with `RtcStats` ([#90]);
    - Handling of RPC events:
        - `TracksApplied` with `TrackUpdate::Added`, `TrackUpdate::Updated` and `TrackUpdate::IceRestart` ([#105], [#138]);
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
[#127]: /../../pull/127
[#132]: /../../pull/132
[#137]: /../../pull/137
[#138]: /../../pull/138
[#143]: /../../pull/143
[#144]: /../../pull/144
[#145]: /../../pull/145
[#146]: /../../pull/146
[#147]: /../../pull/147
[#155]: /../../pull/155




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
