`medea` changelog
=================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## TBD [0.2.0] · 2019-??-??
[0.2.0]: /../../tree/medea-0.2.0

[Milestone](/../../milestone/2) | [Roadmap](/../../issues/27)

### Added

- Control API:
    - Parse static control api specs ([#28]);
    - Created interior entities for control API specs ([#28]).
    - Dynamic control API exposed via gRPC (#[33](/../../pull/33)):
        - `Create` method `Room`, `Member`, `Endpoint`;
        - `Get` method for `Room`, `Member`, `Endpoint`;
        - `Delete` method for `Room`, `Member`, `Endpoint.
- Signalling:
    - Dynamic `Peer`s creation when client connects ([#28]);
    - Auto-removing `Peer`s when `Member` disconnects ([#28]).
- Testing:
    - E2E tests for signalling ([#28]).

[#28]: /../../pull/28




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
