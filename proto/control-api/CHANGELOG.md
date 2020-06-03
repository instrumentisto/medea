`medea-control-api-proto` changelog
===================================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## TBD [0.1.0] · 2019-??-??
[0.1.0]: /../../tree/medea-control-api-proto-0.1.0/proto/control-api

[Milestone](/../../milestone/2) | [Roadmap](/../../issues/27)

### Added

- gRPC:
    - Services:
        - `ControlApi` ([#57]);
        - `Callback` ([#63]).
    - `ControlApi` service:
        - Methods ([#57]):
            - `Create`;
            - `Get`;
            - `Delete`.
        - Elements ([#57], [#79], [#106]):
            - `Room`;
            - `Member`;
            - `WebRtcPlayEndpoint`;
            - `WebRtcPublishEndpoint`.
    - `Callback` service:
        - Callbacks ([#63]):
            - `OnJoin`;
            - `OnLeave`.

[#57]: /../../pull/57
[#63]: /../../pull/63
[#79]: /../../pull/79
[#106]: /../../pull/106





[Semantic Versioning 2.0.0]: https://semver.org
