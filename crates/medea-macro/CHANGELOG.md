`medea-macro` changelog
=======================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## [0.3.0] · ????-??-??
[0.3.0]: /../../tree/medea-macro-0.3.0/crates/medea-macro

[Diff](/../../compare/medea-macro-0.2.1...medea-macro-0.3.0)

### BC breaks

- `#[derive(JsCaused)]` ([#214]):
    - Renamed to `#[derive(Caused)]`;
    - `#[js(cause)]` renamed to `#[cause]`;
    - `#[js(error = "...")]` renamed to `#[cause(error = "...")]`.

- Rename `#[derive(JsCaused)]` macro to `#[derive(Caused)]` ([#214])

[#214]: /../../pull/214




## [0.2.1] · 2021-04-09
[0.2.1]: /../../tree/medea-macro-0.2.1/crates/medea-macro

[Diff](/../../compare/medea-macro-0.2.0...medea-macro-0.2.1)

### Updated

- Switch to [v2 Cargo feature resolver][021-1] ([aa10b2e9]).

[aa10b2e9]: /../../commit/aa10b2e9fc151465f77dc37d7f11f7cf654dbe6f
[021-1]: https://doc.rust-lang.org/cargo/reference/features.html#feature-resolver-version-2




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
