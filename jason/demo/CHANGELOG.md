`medea-demo` changelog
======================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## [0.1.0-rc.1] · 2021-02-02
[0.1.0-rc.1]: /../../tree/medea-demo-0.1.0-rc.1/jason/demo

### Added

- UI/UX:
    - Multiple room members ([#38]);
    - Multiple rooms ([#147]);
    - Audio/video device selection ([#38]);
    - Nickname specifying ([#38]);
    - Muting audio/video tracks ([#156]);
    - Disabling audio/video send/recv tracks ([#40], [#127], [#155]);
    - Connection state indication ([#75]);
    - Call quality indication ([#132]);
    - Force relaying via [TURN] server ([#79]);
    - Screen sharing ([#144]).
- Deployment:
    - [Docker] image ([#38]);
    - [Helm] chart ([#41]).

[#38]: /../../pull/38
[#40]: /../../pull/40
[#41]: /../../pull/41
[#75]: /../../pull/75
[#79]: /../../pull/79
[#127]: /../../pull/127
[#132]: /../../pull/132
[#144]: /../../pull/144
[#147]: /../../pull/147
[#155]: /../../pull/155
[#156]: /../../pull/156





[Docker]: https://docker.io
[Helm]: https://helm.sh
[Semantic Versioning 2.0.0]: https://semver.org
[TURN]: https://webrtc.org/getting-started/turn-server
