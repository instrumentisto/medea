Change Log
==========

All user visible changes to this project will be documented in this file. This project uses to [Semantic Versioning 2.0.0].




## TBD [0.2.0] · 2019-?-?
[0.2.0]: https://github.com/instrumentisto/medea/releases/tag/medea-0.2.0

[Milestone](https://github.com/instrumentisto/medea/milestone/2) |
[Roadmap](https://github.com/instrumentisto/medea/issues/27)

### Added

- Static control API spec [#28](https://github.com/instrumentisto/medea/pull/28)
  - parse static control api specs
  - created interior entities for control API specs
  - dynamically `Peer`s creation when client connects
  - auto removing `Peer`s when `Member` disconnects
  - E2E tests for signalling
  




## [0.1.0] · 2019-08-13
[0.1.0]: https://github.com/instrumentisto/medea/releases/tag/medea-0.1.0

[Milestone](https://github.com/instrumentisto/medea/milestone/1) |
[Roadmap](https://github.com/instrumentisto/medea/issues/8)

### Added

- Structured logging [#12](https://github.com/instrumentisto/medea/pull/12).
- Application configuration [#15](https://github.com/instrumentisto/medea/pull/15).
- [WebRTC signalling] [#16](https://github.com/instrumentisto/medea/pull/16).
- [Coturn] integration [#20](https://github.com/instrumentisto/medea/pull/20), 
  [#42](https://github.com/instrumentisto/medea/pull/42).
- Dockerized medea [#35](https://github.com/instrumentisto/medea/pull/35).
- Graceful shutdown [#30](https://github.com/instrumentisto/medea/pull/30).




[Coturn]: https://github.com/coturn/coturn
[Semantic Versioning 2.0.0]: https://semver.org
