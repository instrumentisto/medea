Medea's Control API mock server
===============================

This app is used for E2E tests and for debugging purposes of [Medea]'s [Control API].




## Endpoints


### `GET /{room_id}`

Get `Room` element.


### `GET /{room_id}/{element_id}`

Get `Room`'s element.
Atm this endpoint can only get `Member` element.


### `GET /{room_id/{element_id}/{endpoint_id}`

Get single `Endpoint` element.


### `POST /{room_id}`

Create `Room` element.


### `POST /{room_id}/{element_id}`

Create element in `Room`.


### `POST /{room_id}/{element_id}/{endpoint_id}`

Create `Endpoint` element.


### `DELETE /{room_id}`

Delete `Room` element.


### `DELETE /{room_id}/{element_id}`

Delete element from `Room`.


### `DELETE /{room_id}/{element_id}/{endpoint_id}`

Delete single `Endpoint` element.





[Medea]: https://github.com/instrumentisto/medea
[Control API]: https://github.com/instrumentisto/medea/blob/master/docs/rfc/0001-control-api.md
