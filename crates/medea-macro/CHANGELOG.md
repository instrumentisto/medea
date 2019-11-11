`medea-macro` changelog
=======================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## TBD [0.2.0] · 2019-??-??
[0.2.0]: /../../tree/medea-macro-0.2.0/crates/medea-macro

### BC Breaks

- Handler traits of `#[dispatchable]` macro now have `type Output`.

### Changed

- `#[dispatchable]` macro generates `type Output` in handler traits with
   which we can specify output type for all functions of handler trait.




## [0.1.0] · 2019-08-21
[0.1.0]: /../../tree/medea-macro-0.1.0/crates/medea-macro

### Added

- `#[enum_delegate]` macro for delegating function calls to `enum` variants fields ([#23](/../../pull/23));
- `#[dispatchable]` macro for dispatching `enum`-based events ([#26](/../../pull/26)).





[Semantic Versioning 2.0.0]: https://semver.org
