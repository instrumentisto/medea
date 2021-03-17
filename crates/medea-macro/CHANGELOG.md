`medea-macro` changelog
=======================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## [0.2.0] · 2021-02-01
[0.2.0]: /../../tree/medea-macro-0.2.0/crates/medea-macro

[Diff](/../../compare/medea-macro-0.1.0...medea-macro-0.2.0)

### BC Breaks

- `#[dispatchable]` macro:
    - Handler traits now require specifying `Output` associative type, which is the return type of all handler trait methods ([#66]).

### Added

- `#[derive(JsCaused)]` macro for deriving `JsCaused` trait from `medea-jason` crate ([#68]).
- `#[dispatchable]` macro:
    - Optional argument to specify `self` type for methods of `*Handler` trait (e.g. `#[dispatchable(self: &Self)]`) ([#112]);
    - Optional argument that enables [async-trait] integration (e.g. `#[dispatchable(async_trait(?Send))]`) ([#112]).
- `#[watchers]` macro for generating `Component::spawn` method in `medea-jason` crate ([#169]).

### Fixed

- `#[enum_delegate]` macro now works fine on functions with multiple arguments ([#91]);
- `#[dispatchable]` handler trait visibility now corresponds to original enum visibility ([#147]).

[#66]: /../../pull/66
[#68]: /../../pull/68
[#91]: /../../pull/91
[#112]: /../../pull/112
[#147]: /../../pull/147
[#169]: /../../pull/169




## [0.1.0] · 2019-08-21
[0.1.0]: /../../tree/medea-macro-0.1.0/crates/medea-macro

### Added

- `#[enum_delegate]` macro for delegating function calls to `enum` variants fields ([#23]);
- `#[dispatchable]` macro for dispatching `enum`-based events ([#26]).

[#23]: /../../pull/23
[#26]: /../../pull/26





[async-trait]: https://docs.rs/async-trait
[Semantic Versioning 2.0.0]: https://semver.org
