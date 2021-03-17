`medea-coturn-telnet-client` changelog
======================================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## [0.1.0] · 2021-02-01
[0.1.0]: /../../tree/medea-coturn-telnet-client-0.1.0/crates/medea-coturn-telnet-client

### Added

- Asynchronous [Coturn] client ([#84]).
- Connections pool ([#84]).
- Requests ([#84]):
    - `ps [<username>]`: prints sessions, with optional exact user match;
    - `cs <session-id>`: forcefully cancels session.

[#84]: /../../pull/84





[Semantic Versioning 2.0.0]: https://semver.org
[Coturn]: https://github.com/coturn/coturn
