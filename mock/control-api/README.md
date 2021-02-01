Medea's Control API mock server
===============================

[Docker Hub](https://hub.docker.com/r/instrumentisto/medea-control-api-mock)
| [GitHub Container Registry](https://github.com/orgs/instrumentisto/packages/container/package/medea-control-api-mock)
| [Quay.io](https://quay.io/repository/instrumentisto/medea-control-api-mock)

[Changelog](https://github.com/instrumentisto/medea/blob/master/mock/control-api/CHANGELOG.md)

This app is used for E2E tests and for debugging purposes of [Medea]'s [Control API].




## Endpoints


### `GET /control-api/{room_id}`

Get `Room` element.


### `GET /control-api/{room_id}/{element_id}`

Get `Room`'s element.
Atm this endpoint can only get `Member` element.


### `GET /control-api/{room_id/{element_id}/{endpoint_id}`

Get single `Endpoint` element.


### `POST /control-api/{room_id}`

Create `Room` element.


### `POST /control-api/{room_id}/{element_id}`

Create element in `Room`.


### `POST /control-api/{room_id}/{element_id}/{endpoint_id}`

Create `Endpoint` element.


### `DELETE /control-api/{room_id}`

Delete `Room` element.


### `DELETE /control-api/{room_id}/{element_id}`

Delete element from `Room`.


### `DELETE /control-api/{room_id}/{element_id}/{endpoint_id}`

Delete single `Endpoint` element.


### `GET /callbacks`

Get list of all `Callback`s that [Control API] mock server received from [Medea].


### `GET /subscribe/{room_id}`

Establish [WebSocket] connection, subscribing to all mutations applied to the selected `Room`. 

This way you can be notified whenever the `Room` state is being updated using current instance of [Control API] mock server. This may be useful in case you are implementing `caller-responder` scenario, meaning that `caller` initiates a call, and `responder` is being notified about that.

Currently, it supports two kinds of events (`Created` and `Deleted`) with the following format:

#### 1. `Created` event

```json
{
  "method": "Created",
  "fid": "room_id/member_id",
  "element": {
    "kind":"Member",
    ...
  } 
}
```

#### 2. `Deleted` event

```json
{
  "method": "Deleted",
  "fid": "room_id/member_id"
}
```

#### 3. `Broadcast` event

Additionally, [WebSocket] clients can send arbitrary messages, and those will be broadcast to other [WebSocket] clients that subscribed to the same `room_id`. The only validation that app performs is that message is a valid JSON.

```json
{
    "method": "Broadcast",
    "payload": {
        "anything": "that other user sent",
        "asd": 123 
    }
}
```




## License

Copyright © 2021 Instrumentisto Team, <https://github.com/instrumentisto>

This software is subject to the terms of the [Blue Oak Model License 1.0.0](https://github.com/instrumentisto/medea/blob/master/mock/control-api/LICENSE.md). If a copy of the [BlueOak-1.0.0](https://spdx.org/licenses/BlueOak-1.0.0.html) license was not distributed with this file, You can obtain one at <https://blueoakcouncil.org/license/1.0.0>.





[Medea]: https://github.com/instrumentisto/medea
[Control API]: https://github.com/instrumentisto/medea/blob/master/docs/rfc/0001-control-api.md
[WebSocket]: https://en.wikipedia.org/wiki/WebSocket
