`medea-jason` changelog
=======================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## [0.3.0] · 2021-??-?? · To-be-done
[0.3.0]: /../../tree/medea-jason-0.3.0/jason

[Diff](/../../compare/medea-jason-0.2.0...medea-jason-0.3.0) | [Milestone](/../../milestone/3) | [Roadmap](/../../issues/182)

### BC Breaks

- Library API:
    - Change `ReconnectHandle.reconnect_with_backoff()` to perform first reconnect attempt immediately ([#206]).

### Added

- Library API:
    - Add optional argument to `ReconnectHandle.reconnect_with_backoff()` function that limits max elapsed time ([#206]).

[#206]: /../../pull/206




## [0.2.0] · 2021-04-09
[0.2.0]: /../../tree/medea-jason-0.2.0/jason

[Diff](/../../compare/medea-jason-0.1.0...medea-jason-0.2.0) | [Milestone](/../../milestone/2) | [Roadmap](/../../issues/27)

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
            - `DeviceVideoTrackConstraints` width and height configuration ([#158]):
                - `DeviceVideoTrackConstraints.ideal_width`;
                - `DeviceVideoTrackConstraints.exact_width`;
                - `DeviceVideoTrackConstraints.width_in_range`;
                - `DeviceVideoTrackConstraints.ideal_height`;
                - `DeviceVideoTrackConstraints.exact_height`;
                - `DeviceVideoTrackConstraints.height_in_range`.
            - `FacingMode` enum ([#137]).
        - `MediaKind` enum that provides `LocalMediaTrack`/`RemoteMediaTrack` and `InputDeviceInfo` kind ([#146]);
        - `MediaSourceKind` enum that provides `MediaTrack` media source kind (`Device` or `Display`) ([#146], [#156]);
        - Room management:
            - `Jason.init_room()` ([#46]);
            - `Room.join()` ([#46]);
            - `Jason.close_room()` ([#147]).
        - Ability to configure local media stream used by `Room` via `Room.set_local_media_settings()` ([#54], [#97], [#145], [#160]):
            - `Room.set_local_media_settings()` can be configured to stop used tracks before trying to acquire new tracks ([#160]);
            - `Room.set_local_media_settings()` can be configured to rollback to previous settings if fail to set new settings ([#160]).
        - `Room.on_failed_local_media` callback ([#54], [#143]);
        - `Room.on_close` callback for WebSocket close initiated by server ([#55]);
        - `RemoteMediaTrack.on_enabled` and `RemoteMediaTrack.on_disabled` callbacks being called when `RemoteMediaTrack` is enabled or disabled ([#123], [#143], [#156]);
        - `RemoteMediaTrack.on_stopped` callback that is called when `RemoteMediaTrack` is stopped ([#109]);
        - `RemoteMediaTrack.on_muted` and `RemoteMediaTrack.on_unmuted` callbacks being called when `RemoteMediaTrack` is muted or unmuted ([#191]);
        - `RemoteMediaTrack.muted()` method indicating whether this `RemoteMediaTrack` is muted ([#191]);
        - `ConnectionHandle.on_remote_track_added` callback being called when new receiver `RemoteMediaTrack` is added ([#123], [#143], [#156]);
        - Enabling/disabling remote video/audio ([#127], [#155]):
            - `Room.disable_remote_audio`;
            - `Room.enable_remote_audio`;
            - `Room.disable_remote_video`;
            - `Room.enable_remote_video`.
        - Muting/unmuting audio/video send ([#156]):
            - `Room.mute_audio`;
            - `Room.unmute_audio`;
            - `Room.mute_video`;
            - `Room.unmute_video`.
        - `RemoteMediaTrack`/`LocalMediaTrack` `media_source_kind` function ([#145], [#146], [#156]);
        - `RemoteMediaTrack` class ([#156]);
        - `LocalMediaTrack` class ([#156]).
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
    - `RpcClient` and `RpcTransport` reconnection ([#75]);
    - State synchronization on a RPC reconnection ([#167]).
- Signalling:
    - Emitting of RPC commands:
        - `AddPeerConnectionMetrics` with `IceConnectionState` and `PeerConnectionState` ([#71], [#87]);
        - `AddPeerConnectionStats` with `RtcStats` ([#90]);
        - Enabling/disabling audio/video send/receive via `UpdateTracks` command ([#81], [#155]);
        - Muting/unmuting audio/video send via `UpdateTracks` ([#156]).
    - Handling of RPC events:
        - `PeerUpdated` with `PeerUpdate::Added`, `PeerUpdate::Updated`, `PeerUpdate::IceRestart` and `PeerUpdate::Removed` ([#105], [#138], [#139], [#109]);
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
[#109]: /../../pull/109
[#120]: /../../pull/120
[#123]: /../../pull/123
[#124]: /../../pull/124
[#127]: /../../pull/127
[#132]: /../../pull/132
[#137]: /../../pull/137
[#138]: /../../pull/138
[#139]: /../../pull/139
[#143]: /../../pull/143
[#144]: /../../pull/144
[#145]: /../../pull/145
[#146]: /../../pull/146
[#147]: /../../pull/147
[#155]: /../../pull/155
[#156]: /../../pull/156
[#158]: /../../pull/158
[#160]: /../../pull/160
[#167]: /../../pull/167
[#191]: /../../pull/191




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
