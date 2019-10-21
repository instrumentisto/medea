Medea's control API mock server
===============================

This app supposed to be used for E2E tests and for debugging purposes of [Medea]'s [Control API].




## Endpoints


### `GET /{room_id}`

Get `Room` element.


### `DELETE /{room_id}`

Delete `Room` element.


### `POST /`

Create `Room` element.


### `GET /{room_id}/{element_id}`

Get `Room`'s element.
Atm this endpoint can only get `Member` element.


### `DELETE /{room_id}/{element_id}`

Delete element from `Room`.


### `POST /{room_id}`

Create element in `Room`.


### `GET /{room_id/{element_id}/{endpoint_id}`

Get single `Endpoint` element.


### `DELETE /{room_id}/{element_id}/{endpoint_id}`

Delete single `Endpoint` element.


### `POST /{room_id}/{element_id}`

Create `Endpoint` element.




[Medea]: https://github.com/instrumentisto/medea
[Control API]: https://tinyurl.com/yxsqplq7
