`medea` changelog
=================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## TBD [0.2.0] · 2020-??-??
[0.2.0]: /../../tree/medea-0.2.0

[Milestone](/../../milestone/2) | [Roadmap](/../../issues/27)

### BC Breaks

- Configuration:
    - Rename `[server]` section of Client API HTTP server as `[server.client.http]` ([#33]).
- RPC messaging:
    - Reverse `Ping`/`Pong` naming: server sends `Ping` and expects `Pongs` from client now. ([#75]).

### Added

- Control API:
    - Support for static Сontrol API specs ([#28]);
    - Dynamic Control API exposed via gRPC ([#33]):
        - `Create` method for `Room`, `Member`, `Endpoint`;
        - `Get` method for `Room`, `Member`, `Endpoint`;
        - `Delete` method for `Room`, `Member`, `Endpoint`.
    - gRPC Control API callbacks ([#63]):
        - `on_join`;
        - `on_leave`.
    - Configuration of `Member`'s Client API RPC settings ([#95]).
- Signalling:
    - Dynamic `Peer`s creation when client connects ([#28]);
    - Auto-removing `Peer`s when `Member` disconnects ([#28]);
    - Filter `SetIceCandidate` messages without `candidate` ([#50]);
    - Send reason of closing WebSocket connection as [Close](https://tools.ietf.org/html/rfc4566#section-5.14) frame's description ([#58]);
    - Send `Event::RpcSettingsUpdated` when `Member` connects ([#75]);
    - Send relay mode in `Event::PeerCreated` which is used for configuring client's `RtcIceTransportPolicy` ([#79]);
    - Send `Command::UpdateTracks` on `Event::TracksUpdated` ([#81]).
- [Coturn] integration:
    - [Coturn] sessions destroying ([#84]);
    - [Coturn] stats processing ([#94]).
- Configuration:
    - `[server.control.grpc]` section to configure Control API gRPC server ([#33]);
    - `[turn.cli]` and `[turn.cli.pool]` sections to configure access to [Coturn] admin interface ([#84]);
    - `server.client.http.public_url` option to configure public URL of Client API HTTP server ([#33]);
    - `rpc.ping_interval` option to configure `Ping`s sending interval ([#75]).
    - `[peer_media_traffic]` ([#98]):
      - `peer_validity_timeout`;
      - `traffic_flowing_timeout`;
      - `peer_init_timeout`.
- Testing:
    - E2E tests for signalling ([#28]).

### Fixed

- Signalling:
    - Room crashing when handling commands with non-existent `peer_id` ([#86]).

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
[Semantic Versioning 2.0.0]: https://semver.org
