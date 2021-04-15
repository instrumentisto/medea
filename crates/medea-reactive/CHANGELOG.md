`medea-reactive` changelog
==========================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## [0.1.1] · 2021-04-09
[0.1.1]: /../../tree/medea-reactive-0.1.1/crates/medea-reactive

[Diff](/../../compare/medea-reactive-0.1.0...medea-reactive-0.1.1)

### Updated

- Switch to [v2 Cargo feature resolver][011-1] ([aa10b2e9]).

[aa10b2e9]: /../../commit/aa10b2e9fc151465f77dc37d7f11f7cf654dbe6f
[011-1]: https://doc.rust-lang.org/cargo/reference/features.html#feature-resolver-version-2




## [0.1.0] · 2021-02-01
[0.1.0]: /../../tree/medea-reactive-0.1.0/crates/medea-reactive

### Added

- `ObservableField` and `ObservableCell` containers ([#81]);
- `Observable` type alias ([#81]);
- `OnObservableFieldModification`, `Subscribable` and `Whenable` traits ([#81]);
- Observable collections ([#103]):
    - `ObservableVec`;
    - `ObservableHashMap`;
    - `ObservableHashSet`.
- `Progressable` container ([#159]);
- Progressable collections ([#159]):
    - `ProgressableVec`;
    - `ProgressableHashMap`;
    - `ProgressableHashSet`.
- `ProgressableCell` container ([#170]);
- `Processed` and `AllProcessed` futures ([#170]).

[#81]: /../../pull/81
[#103]: /../../pull/103
[#159]: /../../pull/159
[#170]: /../../pull/170





[Semantic Versioning 2.0.0]: https://semver.org
