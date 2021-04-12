`medea-coturn-telnet-client` changelog
======================================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## [0.1.1] · 2021-04-09
[0.1.1]: /../../tree/medea-coturn-telnet-client-0.1.1/crates/medea-coturn-telnet-client

[Diff](/../../compare/medea-coturn-telnet-client-0.1.0...medea-coturn-telnet-client-0.1.1)

### Updated

- Switch to [v2 Cargo feature resolver][011-1] ([aa10b2e9]).

[aa10b2e9]: /../../commit/aa10b2e9fc151465f77dc37d7f11f7cf654dbe6f
[011-1]: https://doc.rust-lang.org/cargo/reference/features.html#feature-resolver-version-2




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
