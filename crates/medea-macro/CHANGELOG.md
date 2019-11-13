`medea-macro` changelog
=======================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## TBD [0.2.0] · 2019-??-??
[0.2.0]: /../../tree/medea-macro-0.2.0/crates/medea-macro

### BC Breaks

- `#[dispatchable]` macro:
    - Handler traits now require specifying `Output` associative type, which is the return type of all handler trait methods ([#66](/../../pull/66)).




## [0.1.0] · 2019-08-21
[0.1.0]: /../../tree/medea-macro-0.1.0/crates/medea-macro

### Added

- `#[enum_delegate]` macro for delegating function calls to `enum` variants fields ([#23](/../../pull/23));
- `#[dispatchable]` macro for dispatching `enum`-based events ([#26](/../../pull/26)).





[Semantic Versioning 2.0.0]: https://semver.org
