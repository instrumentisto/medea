Medea's control API mock server
===============================

This app supposed to be used for E2E tests and for debugging purposes of [Medea]'s [Control API].




## Endpoints


### `GET /hb`

Responses with `200 OK` if Control API server available otherwise
responses `500 Intenal Server Error`.


### `GET /`

Batch get elements from [Medea].
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

Get single `Room` element.


### `DELETE /{room_id}`

Delete single `Room` element.


### `POST /{room_id}`

Create `Room` element.


### `GET /{room_id}/{element_id}`

Single get `Room`'s element.
Atm this endpoint can only get `Member` element.


### `DELETE /{room_id}/{element_id}`

Single delete element from `Room`.


### `POST /{room_id}/{element_id}`

Create some element in `Room`.


### `GET /{room_id/{element_id}/{endpoint_id}`

Get single `Endpoint` element.


### `DELETE /{room_id}/{element_id}/{endpoint_id}`

Delete single `Endpoint` element.


### `POST /{room_id}/{element_id}/{endpoint_id}`

Create `Endpoint` element.




[Medea]: https://github.com/instrumentisto/medea
[Control API]: https://tinyurl.com/yxsqplq7
