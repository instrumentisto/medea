`medea-control-api-mock` changelog
==================================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## [0.2.0] · 2021-??-?? · To-be-done
[0.2.0]: /../../tree/medea-control-api-mock-0.2.0/mock/control-api

### Added

- Endpoints:
    - `PUT /control-api/{room_id}` ([#187]);
    - `PUT /control-api/{room_id}/{element_id}` ([#187]).

[#187]: /../../pull/187




## [0.1.0] · 2021-02-01
[0.1.0]: /../../tree/medea-control-api-mock-0.1.0/mock/control-api

### Added

- Endpoints:
    - `GET /control-api/{room_id}` ([#36]);
    - `GET /control-api/{room_id}/{element_id}` ([#36]);
    - `GET /control-api/{room_id/{element_id}/{endpoint_id}` ([#36]);
    - `POST /control-api/{room_id}` ([#36]);
    - `POST /control-api/{room_id}/{element_id}` ([#36]);
    - `POST /control-api/{room_id}/{element_id}/{endpoint_id}` ([#36]);
    - `DELETE /control-api/{room_id}` ([#36]);
    - `DELETE /control-api/{room_id}/{element_id}` ([#36]);
    - `DELETE /control-api/{room_id}/{element_id}/{endpoint_id}` ([#36]);
    - `GET /callbacks` ([#36], [#63]);
    - `GET /subscribe/{room_id}` ([#118], [#136]).
- Events:
    - `Created` ([#118]);
    - `Deleted` ([#118]);
    - `Broadcast` ([#136]).

[#36]: /../../pull/36
[#63]: /../../pull/63
[#118]: /../../pull/118
[#136]: /../../pull/136





[Semantic Versioning 2.0.0]: https://semver.org
