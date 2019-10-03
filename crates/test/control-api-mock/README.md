Medea's control API mock server
===============================

This app supposed to be used for e2e tests and for debugging purposes.




## Endpoints

### `GET /hb`
Checks connection with medea's gRPC control API.

This is used for waiting before e2e tests start until all needed services
startup.

Returns `200 OK` with body "Ok." when gRPC control API go online.
=======

>>>>>>> control-api-mock-server:control-api-mock/README.md

### `GET /`

Batch get elements from medea.
_Elements can be heterogeneous._

| name | description                       |
|:-----|:----------------------------------|
| ids  | Elements IDs which we want to get |

#### Example request

```json
{
  "ids": [
    "local://video-call-1",
    "local://video-call-1/caller",
    "local://video-call-1/caller/play"
  ]
}
```


### `DELETE /`

Batch delete elements from medea.
_Elements can be heterogeneous._

| name | description            |
|:-----|:-----------------------|
| ids  | Elements IDs to delete |

#### Example request

```json
{
  "ids": [
    "local://video-call-1",
    "local://video-call-1/caller",
    "local://video-call-1/caller/play"
  ]
}
```


### `GET /{room_id}`

Single get `Room` element.


### `DELETE /{room_id}`

Single delete `Room` element.


### `POST /{room_id}`

Create `Room` element.


### `GET /{room_id}/{element_id}`

Single get `Room`'s element.
Atm this endpoint can only get `Member` element.


### `DELETE /{room_id}/{element_id}`

Single delete element from `Room`.


### `POST /{room_id}/{element_id}`

Create some element.


### `GET /{room_id/{element_id}/{endpoint_id}`

Single get `Endpoint` element.


### `DELETE /{room_id}/{element_id}/{endpoint_id}`

Single delete `Endpoint` element.


### `POST /{room_id}/{element_id}/{endpoint_id}`

Create `Endpoint` element.
