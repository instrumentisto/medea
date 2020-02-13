`medea-coturn-telnet` changelog
=======================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## TBD [0.1.0] · 2019-??-??
[0.1.0]: /../../tree/medea-coturn-telnet-0.1.0/crates/medea-coturn-telnet

### Added

- Asynchronous [Coturn] client ([#84]).
- Connection pool ([#84]).
- Requests ([#84]):
    - `ps [username]`, that prints sessions, with optional exact user match.
    - `cs <session-id>`, that forcefully cancels session.

[#28]: /../../pull/84





[Semantic Versioning 2.0.0]: https://semver.org
[Coturn]: https://github.com/coturn/coturn
