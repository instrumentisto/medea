`medea` changelog
=================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## [0.3.0] · 2021-??-??
[0.3.0]: /../../tree/medea-0.3.0

[Diff](/../../compare/medea-0.2.0...medea-0.3.0) | [Milestone](/../../milestone/3) | [Roadmap](/../../issues/182)

### BC Breaks

- Configuration:
  - Move `[turn]` section to `[turn.coturn]` ([#211]).
    
### Added
- Configuration:
    - `turn.is_static` option to configure [TURN]/[STUN] server mode ([#211]);
    - `[[turn.static]]` option to configure static [TURN]/[STUN] servers credentials ([#211]);

[#211]: /../../pull/211




## [0.2.0] · 2021-04-09
[0.2.0]: /../../tree/medea-0.2.0

[Diff](/../../compare/medea-0.1.0...medea-0.2.0) | [Milestone](/../../milestone/2) | [Roadmap](/../../issues/27)

### BC Breaks

- Configuration:
    - Rename `[server]` section of Client API HTTP server as `[server.client.http]` ([#33]).
- RPC messaging:
    - Reverse `Ping`/`Pong` naming: server sends `Ping` and expects `Pongs` from client now. ([#75]).

### Added

- Control API:
    - Support for static Сontrol API specs ([#28]);
    - Dynamic Control API exposed via gRPC:
        - `Create` method for `Room`, `Member`, `Endpoint` ([#33]);
        - `Get` method for `Room`, `Member`, `Endpoint` ([#33]);
        - `Delete` method for `Room`, `Member`, `Endpoint` ([#33]);
        - `Apply` method for `Room`, `Member`, `Endpoint` ([#187]).
    - gRPC Control API callbacks:
        - `on_join` ([#63], [#153]);
        - `on_leave` ([#63]).
    - Configuration of `Member`'s Client API RPC settings ([#95]);
    - Hashed `Member` credentials support ([#168]).
- Signalling:
    - Dynamic `Peer`s creation when client connects ([#28]);
    - Auto-removing `Peer`s when `Member` disconnects ([#28]);
    - Filter `SetIceCandidate` messages without `candidate` ([#50]);
    - Send reason of closing WebSocket connection as [Close](https://tools.ietf.org/html/rfc4566#section-5.14) frame's description ([#58]);
    - Send `Event::RpcSettingsUpdated` when `Member` connects ([#75]);
    - Send relay mode in `Event::PeerCreated` which is used for configuring client's `RtcIceTransportPolicy` ([#79]);
    - Emit `PeerUpdated` event to create new and update existing tracks ([#105], [#139]);
    - Emit `TracksApplied` event to remove existing tracks on a client side ([#109]);
    - `PeerConnection` renegotiation functionality ([#105]);
    - Calculate and send call quality score based on RTC stats ([#132]);
    - Enabling/disabling `MediaTrack`s by receiver ([#127], [#155]);
    - Send `PeerUpdate::IceRestart` based on RTC stats analysis ([#138], [#139]);
    - Multiple `Room`s served by one RPC connection support ([#147]);
    - Muting/unmuting `MediaTrack`s ([#156]);
    - State synchronization on a RPC reconnection ([#167]).
- [Coturn] integration:
    - [Coturn] sessions destroying ([#84]);
    - [Coturn] stats processing ([#94]).
- Configuration:
    - `[server.control.grpc]` section to configure Control API gRPC server ([#33]);
    - `[turn.cli]` and `[turn.cli.pool]` sections to configure access to [Coturn] admin interface ([#84]);
    - `server.client.http.public_url` option to configure public URL of Client API HTTP server ([#33]);
    - `rpc.ping_interval` option to configure `Ping`s sending interval ([#75]);
    - `[media]` section to configure timeouts involved for determining media flow liveness ([#98]):
        - `max_lag`;
        - `init_timeout`.
    - `turn.db.redis.user` option to configure user to authenticate on [Coturn]'s [Redis] database server as ([#135]).
- Testing:
    - E2E tests for signalling ([#28]).

### Fixed

- Signalling:
    - Room crashing when handling commands with non-existent `peer_id` ([#86]);
    - Adding new endpoints to the already interconnected `Member`s ([#105]).

[#28]: /../../pull/28
[#33]: /../../pull/33
[#50]: /../../pull/50
[#58]: /../../pull/58
[#63]: /../../pull/63
[#75]: /../../pull/75
[#79]: /../../pull/79
[#81]: /../../pull/81
[#84]: /../../pull/84
[#86]: /../../pull/86
[#94]: /../../pull/94
[#95]: /../../pull/95
[#98]: /../../pull/98
[#105]: /../../pull/105
[#109]: /../../pull/109
[#127]: /../../pull/127
[#132]: /../../pull/132
[#135]: /../../pull/135
[#138]: /../../pull/138
[#139]: /../../pull/139
[#147]: /../../pull/147
[#153]: /../../pull/153
[#155]: /../../pull/155
[#156]: /../../pull/156
[#167]: /../../pull/167
[#168]: /../../pull/168
[#187]: /../../pull/187




## [0.1.0] · 2019-08-22
[0.1.0]: /../../tree/medea-0.1.0

[Milestone](/../../milestone/1) | [Roadmap](/../../issues/8)

### Added

- WebRTC:
    - Basic signalling capabilities ([#16](/../../pull/16));
    - [Coturn] integration ([#20](/../../pull/20), [#42](/../../pull/42)).
- Deployment:
    - Graceful shutdown ([#30](/../../pull/30));
    - Docker image ([#35](/../../pull/35)).
- Configuration:
    - Ability to parse from files and env vars ([#15](/../../pull/15)).
- Logging:
    - Structured logging to STDOUT/STDERR ([#12](/../../pull/12)).





[Coturn]: https://github.com/coturn/coturn
[Redis]: https://redis.io
[Semantic Versioning 2.0.0]: https://semver.org
