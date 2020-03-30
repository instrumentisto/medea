- Feature Name: `control_api`
- Start Date: 2018-11-21
- RFC PR: [instrumentisto/medea#5](https://github.com/instrumentisto/medea/pull/5)
- Tracking Issue: [instrumentisto/medea#4](https://github.com/instrumentisto/medea/issues/4)




## Summary
[summary]: #summary

Organize a Control API for the media server which is not facing client side and allows media server user (developer) to create any media data pipelines on his decision. 




## Motivation
[motivation]: #motivation

There are many media server implementations and each one has its own interaction model. While the same happens on business logic side too: there is a variety of scenarios of media data usage which does not fit the one sort of interaction model proposed by media server. This leads to the situation when some media server is used in unnatural way for business scenarios, or developer should fork-and-hack inner media server implementation, or even multiple media servers should work in integration to fit business requirements.

Control API is intended to be a sort of "silver bullet" (or rather "swiss army knife"), which allows developer to express almost any desired topology of media data pipeline and fits naturally (or quite close) into his business domain model.

Control API should remove the necessity of using other media servers in most cases, or, regarding the cases where it's not possible, fit well and naturally to the interaction and integration with other media servers.




## Guide-level explanation
[guide-level-explanation]: #guide-level-explanation

Control API listens connections on dedicated port and has no interaction with the client side, but only with the server side of business application.

It has the only main primitive - `Element`, which represents a media element that one or more media data streams flow through. Each `Element` has its own unique ID set by developer, and can be referred with this ID in other `Elements` or operations with media server. All `Element`s are typed by different kinds and each kind encapsulates its own media data flow logic inside (sometimes quite complex). Some `Elements` may allow nesting and may consist of other `Elements`.  
Examples: `WebRtcPublishEndpoint` which receives media data from client via [WebRTC], `HlsPlayEndpoint` which allows client to play media data in [HLS] format, `HubEndpoint` endpoint which multiplexes and routes media data streams dynamically, `Room` which logically groups some set of `Element`s, `Member` which groups some set of `Element` behind a client session, and so on...  
Concrete set of `Element`s for the media server to have is not the topic of this RFC and will be formed during implementation. 

Developer interacts with Control API in a declarative way: it creates, applies or removes `Element`s by providing a desired spec of `Element` and media server "does the rest" (makes changes in existing `Element`s towards the desired spec). It automatically encodes/decodes media data if necessary and tries it's best in data flow optimization, while the developer expresses only the desired topology.

Depending on `Element`'s kind it may register optional callbacks (provided by developer) and invoke them when the relative event happens. This allows server side applications to integrate deeply into lifecycle of a media data pipeline, receiving the events on what is happening on and reacting on them. Potentially, these callbacks may come in many forms ([gRPC], HTTP, raw TCP connection, [Apache Kafka] events, [Kubernetes] events, etc), so the developer may choose the best fit for the desired integration. The behavior of such callbacks entirely depends on the `Element` which they are represented by, so one `Element`s can ignore callback results and provide them only as notification, and other `Element` may depend on callback result.

<details>
<summary>One-to-one P2P WebRTC video call YAML example</summary>

In this example, media server acts only as simple signaling server for [WebRTC], because P2P is required.
```yaml
# We're grouping all media elements of video call into dedicated room for
# better referring and single media data pipeline lifecycle.
kind: Room
id: video-call-2
spec:
  pipeline:
    # Here we're defining a member who initiates video call.
    caller:
      kind: Member
      spec:
        # Fires when "caller" client connects to media server via WebSocket
        # for WebRTC negotiation.
        on_join: "grpc://127.0.0.1:9091"
        # Fires when "caller" client disconnects from media server via WebSocket.
        on_leave: "grpc://127.0.0.1:9091"
        # Duration, after which remote RPC client will be considered idle if no 
        # heartbeat messages received.
        idle_timeout: 1m
        # Duration, after which the server deletes the client session if the remote
        # RPC client does not reconnect after it is idle.
        reconnect_timeout: 3m
        # Interval of sending pings from the server to the client.
        ping_interval: 10s
        pipeline:
          # Media element which is able to receive media data from client via WebRTC.
          publish:
            kind: WebRtcPublishEndpoint
            spec:
              # Actually, it receives not media data, but ICE candidates only.
              p2p: Always
              # Fires when "caller" client starts publishing media data.
              on_start: "http://127.0.0.1:8080/publish/started"
              # Fires when "caller" client stops publishing media data.
              on_stop: "http://127.0.0.1:8080/publish/stopped"
              # All media will be relayed through TURN server.
              force_relay: false
          # Media element which is able to play media data for client via WebRTC.
          play:
            kind: WebRtcPlayEndpoint
            spec:
              # The source to get media data from.
              # It will take responder's ICE candidates and propose them to client side,
              # so the WebRTC negotiation can happen.
              src: "local://video-call-2/responder/publish"
              # Fires when "caller" client starts playing media data of "responder" client.
              on_start: "http://127.0.0.1:8080/play/started"
              # Fires when "caller" client stops playing media data of "responder" client.
              on_stop: "http://127.0.0.1:8080/play/stopped"
    responder:
      kind: Member
      spec:
        on_join: "grpc://127.0.0.1:9091"
        on_leave: "grpc://127.0.0.1:9091"
        pipeline:
          publish:
            kind: WebRtcPublishEndpoint
            spec:
              p2p: Always
              on_start: "http://127.0.0.1:8080/publish/started"
              on_stop: "http://127.0.0.1:8080/publish/stopped"
          play:
            kind: WebRtcPlayEndpoint
            spec:
              src: "local://video-call-2/caller/publish"
              on_start: "http://127.0.0.1:8080/play/started"
              on_stop: "http://127.0.0.1:8080/play/stopped"
```
</details>

After creation of desired media data pipeline via Control API, it can be used from the client side.

Depending on concrete `Element`'s kind, it may implement auto-removing rules, which are going to help avoiding manual `Element`s removing in some situations (remove the whole `Room` automatically when video call ends, for example). These rules may vary, depending on `Element`'s behavior, but commonly have the following semantics (`Element` is auto-removed when):
- all real clients (`Member`s) have left the media server;
- all media data sources have ended.




## Reference-level explanation
[reference-level-explanation]: #reference-level-explanation

For better integration capabilities the Control API should be implemented in two flavors (both can be enabled/disabled/configured via media server configuration):
- *[gRPC] interface*, which easy-to-go due to code generation;
- *HTTP rest interface* for languages and tools which are not supported by [gRPC].

Any method call of Control API is atomic: if it succeeds then all spec changes are applied, otherwise (if errors) none changes must be applied. 


### HTTP REST interface

HTTP REST interface implementation should use a convenient modern tooling for its declarations (such as [Swagger] or its alternatives). This allows further capabilities for code generation (for user side too) and may simplify the bridge between [gRPC] and REST HTTP (via tools like `protoc-gen-swagger`).

#### Body format

Accepted `Content-Type`s:
- `application/json` for JSON;
- `application/yaml`, `application/x-yaml`, `application/vnd.yaml` for YAML;
- `application/msgpack`, `application/x-msgpack`, `application/vnd.msgpack` for [MessagePack].

Body must be encoded according to provided `Content-Type`.

Body contains single `Element`'s spec declaration (but nesting is allowed).

#### Response format

Response is encoded in the format provided by `Content-Type` header of request, so JSON is answered with JSON, YAML with YAML, and so on.

If request contains `Member` `Element`s, then a `sid` (session ID) secret will be returned for each `Member`. `sid` represents an URL that should be used by client side to connect to media server. URLs are formed according to `Element`s IDs hierarchy.

If request fails, then an `error` field will be returned. The request should be considered successful only if its response does not contain `error`. The `error` will contain the following fields:
- `status`: HTTP status code of error, this can be used to react differently onto received errors by their kind;
- `code`: concrete unique code of error;
- `text`: human-readable text description of error;
- `doc` (optional): link to online documentation of error;
- `element`: full ID of `Element` that error is related to;
- `details` (optional): additional structured details related to the error;
- `backtrace` (optional): back trace of nested errors for debugging purposes (enabled via configuration).

Examples:

<details>
<summary>Successful response</summary>

```json
{}
```
</details>

<details>
<summary>Successful response with session IDs</summary>

```json
{
  "sid": {
    "member-1": "wss://my.medea.io/conference-1/member-1?token=XXXXXXXXXXXXXXXX",
    "member-2": "wss://my.medea.io/conference-1/member-1?token=YYYYYYYYYYYYYYYY"
  }
}
```
</details>

<details>
<summary>Error response</summary>

```json
{
  "error": {
    "status": 404,
    "code": 10567,
    "text": "No element exists for given ID",
    "doc": "https://doc.medea.io/errors/#10567",
    "element": "video-call-1/arbiter"
  }
}
```
</details>

#### Methods

##### `POST /{element-id}[/{sub-element-id[/{sub-element-id}]}]`

Creates new `Element` with given ID.

Not idempotent. Errors if `Element` with the same ID already exists.

Examples:

<details>
<summary>Create P2P WebRTC video call</summary>

```http request
POST /video-call-2
Content-Type: application/yaml
```
```yaml
kind: Room
spec:
  pipeline:
    caller:
      kind: Member
      spec:
        on_join: "grpc://127.0.0.1:9091"
        on_leave: "grpc://127.0.0.1:9091"
        pipeline:
          publish:
            kind: WebRtcPublishEndpoint
            spec:
              p2p: Always
              on_start: "http://127.0.0.1:8080/publish/started"
              on_stop: "http://127.0.0.1:8080/publish/stopped"
          play:
            kind: WebRtcPlayEndpoint
            spec:
              src: "local://video-call-2/responder/publish"
              on_start: "http://127.0.0.1:8080/play/started"
              on_stop: "http://127.0.0.1:8080/play/stopped"
    responder:
      kind: Member
      spec:
        on_join: "grpc://127.0.0.1:9091"
        on_leave: "grpc://127.0.0.1:9091"
        pipeline:
          publish:
            kind: WebRtcPublishEndpoint
            spec:
              p2p: Always
              on_start: "http://127.0.0.1:8080/publish/started"
              on_stop: "http://127.0.0.1:8080/publish/stopped"
          play:
            kind: WebRtcPlayEndpoint
            spec:
              src: "local://video-call-2/caller/publish"
              on_start: "http://127.0.0.1:8080/play/started"
              on_stop: "http://127.0.0.1:8080/play/stopped"
```

Response:
```yaml
sid:
  caller: wss://my.medea.io/video-call-2/caller/?token=XXXXXX
  responder: wss://my.medea.io/video-call-2/caller/?token=XXXXXX
```
</details>

<details>
<summary>Add new member to WebRTC conference</summary>

Omitted `spec` fields evaluate do their defaults.

Errors if `conference-2ab3ef` `Room` does not exist.

```http request
POST /conference-2ab3ef/member-23ac4
Content-Type: application/json
```
```json
{
  "kind": "Member",
  "spec": {
    "on_leave": "http://127.0.0.1:8080/member/left",
    "pipeline": {
      "publish": {
        "kind": "WebRtcPublishEndpoint",
        "spec": {
          "p2p": "Never",
          "dst": "self://../hub"
        }
      },
      "play": {
        "kind": "WebRtcPlayEndpoint",
        "spec": {
          "src": "self://../hub"
        }
      }
    }
  }
}
```

Response:
```json
{
  "sid": {
    "member-23ac4": "wss://my.medea.io/conference-2ab3ef/member-23ac4/?token=XXXXXX"
  }
}
```
</details>

<details>
<summary>Enable broadcasting recording on-fly</summary>

Omitted `spec` fields evaluate do their defaults.

Errors if `broadcast-1` `Room` does not exist, or has no `publisher` `Member`.

```http request
POST /broadcast-1/publisher/recorder
Content-Type: application/json
```
```json
{
  "kind": "FileRecorder",
  "spec": {
    "src": "self://webrtc",
    "dst": "file:///data/recorded/broadcast-1.mkv"
  }
}
```

Response:
```json
{}
```
</details>

##### `PUT /{element-id}[/{sub-element-id[/{sub-element-id}]}]?policy=(apply|append)`

Applies given spec to `Element` by its ID.

Idempotent. If no `Element` with such ID exists, then it will be created, otherwise it will be reconfigured.

The default behavior is "apply" (`policy=apply`): `Element`s that exist, but are not specified in provided `pipeline` will be removed. To enable "append-only" behavior the `policy=append` URL parameter must be provided.

Examples:

<details>
<summary>Convert P2P WebRTC video call into non-P2P</summary>

```http request
PUT /video-call-2
Content-Type: application/yaml
```
```yaml
kind: Room
spec:
  pipeline:
    caller:
      kind: Member
      spec:
        on_join: "grpc://127.0.0.1:9091"
        on_leave: "grpc://127.0.0.1:9091"
        pipeline:
          publish:
            kind: WebRtcPublishEndpoint
            spec:
              p2p: Never
              on_start: "http://127.0.0.1:8080/publish/started"
              on_stop: "http://127.0.0.1:8080/publish/stopped"
          play:
            kind: WebRtcPlayEndpoint
            spec:
              src: "local://video-call-2/responder/publish"
              on_start: "http://127.0.0.1:8080/play/started"
              on_stop: "http://127.0.0.1:8080/play/stopped"
    responder:
      kind: Member
      spec:
        on_join: "grpc://127.0.0.1:9091"
        on_leave: "grpc://127.0.0.1:9091"
        pipeline:
          publish:
            kind: WebRtcPublishEndpoint
            spec:
              p2p: Never
              on_start: "http://127.0.0.1:8080/publish/started"
              on_stop: "http://127.0.0.1:8080/publish/stopped"
          play:
            kind: WebRtcPlayEndpoint
            spec:
              src: "local://video-call-2/caller/publish"
              on_start: "http://127.0.0.1:8080/play/started"
              on_stop: "http://127.0.0.1:8080/play/stopped"
```

Response:
```yaml
sid:
  caller: wss://my.medea.io/video-call-2/caller/?token=XXXXXX
  responder: wss://my.medea.io/video-call-2/caller/?token=XXXXXX
```
Here `sid`s will remain the same as these `Member`s where already created before.
</details>

<details>
<summary>Make member of WebRTC conference a watch-only</summary>

This will remove `publish` `Element` for `member-23ac4` member.

Errors if `conference-2ab3ef` `Room` does not exist.

```http request
PUT /conference-2ab3ef/member-23ac4
Content-Type: application/json
```
```json
{
  "kind": "Member",
  "spec": {
    "on_leave": "http://127.0.0.1:8080/member/left",
    "pipeline": {
      "play": {
        "kind": "WebRtcPlayEndpoint",
        "spec": {
          "src": "self://../hub"
        }
      }
    }
  }
}
```

Response:
```json
{
  "sid": {
    "member-23ac4": "wss://my.medea.io/conference-2ab3ef/member-23ac4/?token=XXXXXX"
  }
}
```
</details>

<details>
<summary>Change broadcasting recording file and add aternative file</summary>

Omitted `spec` fields evaluate do their defaults.

Errors if `broadcast-1` `Room` does not exist.

```http request
POST /broadcast-1/publisher?policy=append
Content-Type: application/json
```
```json
{
  "kind": "Member",
  "spec": {
    "pipeline": {
      "recorder": {
        "kind": "FileRecorder",
        "spec": {
          "src": "self://webrtc",
          "dst": "file:///data/recorded/broadcast-1.main.mkv"
        }
      },
      "recorder-small": {
        "kind": "FileRecorder",
        "spec": {
          "src": "self://webrtc",
          "dst": "file:///data/recorded/broadcast-1.small.mkv?resolution=320x240"
        }
      }
    }
  }
}
```

Response:
```json
{
  "sid": {
    "publisher": "wss://my.medea.io/broadcast-1/publisher/?token=XXXXXX"
  }
}
```
</details>

##### `DELETE /{element-id}[/{sub-element-id[/{sub-element-id}]}]`

Removes `Element` by its ID. Allows referring multiple `Element`s on the last level.

Idempotent.

Examples:

<details>
<summary>Remove the whole WebRTC video call</summary>

```http request
DELETE /video-call-2
```

Response:
```json
{}
```
</details>

<details>
<summary>Remove 3 members from WebRTC conference</summary>

Does not error if `conference-2ab3ef` `Room` does not exist.

```http request
DELETE /conference-2ab3ef/member-23ac4,member-1bf90,member-832ec 
```

Response:
```json
{}
```
</details>

<details>
<summary>Remove broadcasting recording on-fly</summary>

Does not error if `broadcast-1` `Room` does not exist, or has no `publisher` `Member`.

```http request
DELETE /broadcast-1/publisher/recorder,recorder-small
```

Response:
```json
{}
```
</details>

##### `GET /[{element-id}[/{sub-element-id[/{sub-element-id}]}]]`

Returns `Element` by its ID. Allows referring multiple `Element`s on the last level. If no ID specified, returns all `Element`s declared.

If multiple `Element`s requested, the returns only found ones. If no `Element`s found - returns 404 error.

<details>
<summary>Get P2P WebRTC video call spec</summary>

```http request
GET /video-call-2
Content-Type: application/yaml
```

Response:
```yaml
video-call-2:
  kind: Room
  spec:
    pipeline:
      caller:
        kind: Member
        spec:
          on_join: "grpc://127.0.0.1:9091"
          on_leave: "grpc://127.0.0.1:9091"
          pipeline:
            publish:
              kind: WebRtcPublishEndpoint
              spec:
                p2p: Always
                on_start: "http://127.0.0.1:8080/publish/started"
                on_stop: "http://127.0.0.1:8080/publish/stopped"
            play:
              kind: WebRtcPlayEndpoint
              spec:
                src: "local://video-call-2/responder/publish"
                on_start: "http://127.0.0.1:8080/play/started"
                on_stop: "http://127.0.0.1:8080/play/stopped"
      responder:
        kind: Member
        spec:
          on_join: "grpc://127.0.0.1:9091"
          on_leave: "grpc://127.0.0.1:9091"
          pipeline:
            publish:
              kind: WebRtcPublishEndpoint
              spec:
                p2p: Always
                on_start: "http://127.0.0.1:8080/publish/started"
                on_stop: "http://127.0.0.1:8080/publish/stopped"
            play:
              kind: WebRtcPlayEndpoint
              spec:
                src: "local://video-call-2/caller/publish"
                on_start: "http://127.0.0.1:8080/play/started"
                on_stop: "http://127.0.0.1:8080/play/stopped"
```
</details>


<details>
<summary>Get spec of broadcasting recordings</summary>

```http request
GET /broadcast-1/publisher/recorder,recorder-small
```

Response:
```json
{
  "recorder": {
    "kind": "FileRecorder",
    "spec": {
      "src": "self://webrtc",
      "dst": "file:///data/recorded/broadcast-1.main.mkv"
    }
  },
  "recorder-small": {
    "kind": "FileRecorder",
    "spec": {
      "src": "self://webrtc",
      "dst": "file:///data/recorded/broadcast-1.small.mkv?resolution=320x240"
    }
  }
}
```
</details>

#### HTTP callbacks

HTTP callbacks provided in specs (`http://`/`https://` schemes) must meet following conventions for correct integration:
- They should be implemented via `POST` HTTP method (so not idempotent by default);
- On success the `2xx` HTTP status code must be returned;
- Redirects are allowed, and the maximum number of followed redirects can be configured in media server;
- Any other HTTP status code is considered to be an error.

The behavior of media server in response to callback result entirely depends on concrete `Element`'s implementation. Some `Element`s may ignore error result, another ones may finish on error result.

The returned by callback parameters are:
- `element`: full ID of `Element` that callback is related to;
- `event`: name of callback in spec;
- `at`: date and time (in microseconds extended [RFC 3339] format) when this event has happened.

Examples:

<details>
<summary>on_join callback for Member</summary>

Providing the callback:
```yaml
on_join: http://127.0.0.1/
```

Will result in the following HTTP request from media server:
```http request
POST http://127.0.0.1
Content-Type: application/json
```
```json
{
  "element": "video-call-1/caller",
  "event": "on_join",
  "at": "2018-11-22T13:05:32.032412Z"
}
```
</details>

<details>
<summary>Secure on_start callback with authorization in YAML format for WebRtcPublishEndpoint</summary>

Providing the callback:
```yaml
on_start: yaml+https://user:password@my.app.org/callback?my=parameter
```

Will result in the following HTTP request from media server:
```http request
POST https://my.app.org/callback?my=parameter
Content-Type: application/x-yaml
Authorization: <BASE64-encoded-authorization-credentials>
```
```yaml
element: video-call-1/caller/publish
event: on_start
at: 2018-11-22T13:05:32.032412Z
```
</details>


### [gRPC] interface

[gRPC] interface repeats the HTTP REST interface, and may be reproduced with the following spec:

<details>
<summary>gRPC Control API service spec</summary>

```proto
import "google/protobuf/any.proto";

service ControlApi {
  rpc Create (CreateRequest) returns (Response);
  rpc Apply (ApplyRequest) returns (Response);
  rpc Delete (IdRequest) returns (Response);
  rpc Get (IdRequest) returns (GetResponse);
}
message CreateRequest {
  required string id = 1;
  oneof el {
      Hub hub = 2;
      FileRecorder file_recorder = 3;
      Member member = 4;
      Relay relay = 5;
      Room room = 6;
      WebRtcPlayEndpoint webrtc_play = 7;
      WebRtcPublishEndpoint webrtc_pub = 8;
  }
}
message ApplyRequest {
  required string id = 1;
  oneof el {
    Hub hub = 2;
    FileRecorder file_recorder = 3;
    Member member = 4;
    Relay relay = 5;
    Room room = 6;
    WebRtcPlayEndpoint webrtc_play = 7;
    WebRtcPublishEndpoint webrtc_pub = 8;
  }
  optional Policy policy = 9 [default = APPLY];
  
  enum Policy {
    APPLY = 1;
    APPEND = 2;
  }
}
message IdRequest {
  repeated string id = 1;
}
message Response {
  map<string, string> sid = 1;
  optional Error error = 2;
}
message GetResponse {
  map<string, Element> elements = 1;
  optional Error error = 2;
}
message Error {
  required uint32 status = 1;
  required uint32 code = 2;
  required string text = 3;
  optional string doc = 4;
  required string element = 5;
  optional google.protobuf.Any details = 6;
  repeated string backtrace = 7;
}

message Element {
  oneof el {
    Hub hub = 2;
    FileRecorder file_recorder = 3;
    Member member = 4;
    Relay relay = 5;
    Room room = 6;
    WebRtcPlayEndpoint webrtc_play = 7;
    WebRtcPublishEndpoint webrtc_pub = 8;
  }
}
  
message Room {
  map<string, Room.Element> pipeline = 1;
  
  message Element {
    oneof el {
      Hub hub = 1;
      FileRecorder file_recorder = 2;
      Member member = 3;
      Relay relay = 4;
      WebRtcPlayEndpoint webrtc_play = 5;
      WebRtcPublishEndpoint webrtc_pub = 6;
    }
  }
}

message Member {
  optional string on_join = 1;
  optional string on_leave = 2;
  map<string, Member.Element> pipeline = 3;
  optional string credentials = 4;
  optional uint64 idle_timeout = 5;
  optional uint64 reconnect_timeout = 7;
  optional uint64 ping_interval = 8;
  
  message Element {
    oneof el {
      Hub hub = 1;
      FileRecorder file_recorder = 2;
      Relay relay = 3;
      WebRtcPlayEndpoint webrtc_play = 4;
      WebRtcPublishEndpoint webrtc_pub = 5;
    }
  }
}

message WebRtcPublishEndpoint {
  optional P2P p2p = 1 [default = NEVER];
  optional string dst = 2;
  optional string on_start = 3;
  optional string on_stop = 4;
  optional bool force_relay = 5;
  
  enum P2P {
    NEVER = 0;
    IF_POSSIBLE = 1;
    ALWAYS = 2;
  }
}

message WebRtcPlayEndpoint {
  required string src = 1;
  optional string on_start = 2;
  optional string on_stop = 3;
}

message Hub {}

message FileRecorder {
  required string src = 1;
  required string dst = 2;
  optional string on_start = 3;
  optional string on_stop = 4;
}

message Relay {
  required string src = 1;
  optional string dst = 2;
}
```
</details>

#### [gRPC] callbacks

To be able to receive callbacks from media server via [gRPC] (`grpc://` scheme in specs), the calling side of Control API (who uses media server) must implement the server side of `Callback` service, and the media server itself implements the client side of `Callback` service.

The callback mechanism is the pretty same as for HTTP callbacks, but in [gRPC] terms. Instead of HTTP status code, it should react with gRPC error status codes.

<details>
<summary>gRPC Callback service spec</summary>

```proto
service Callback {
  rpc OnEvent (Request) returns (Response);
}

message Request {
  required string element = 1;
  required string event = 2;
  required string at = 3;
}

message Response {}
```
</details>


### Auto-removing rules

Sometimes it's a lot easier to deal with media data pipeline when it can be automatically cleaned up from media server upon its end. However, this is not true for any case (for example, we want to serve static files from some directly via HLS as long as media server is up). That's why this behavior should be part of a concrete `Element` kind, so the developer may choose and build the desired lifecycle.




## Drawbacks
[drawbacks]: #drawbacks

The biggest drawback of introducing Control API is that the additional step is required for media data pipeline initialization (first, pre-create pipeline, and only then interact with client). However, without pre-creation step the developer will be bound to some single interaction model. Fortunately, this drawback can be eliminated with allowing [static media data pipelines][static-declarations] (configured on media server start and exist the whole time media server runs), which are not a topic of this RFC.




## Rationale and alternatives
[rationale-and-alternatives]: #rationale-and-alternatives

This RFC design tries to be a "silver bullet": allow media server user to do anything he wants, yet not be too low-level for the reason to remove complexity and express things in a high-level understandable way for him. Future Control API extension should be easy too.


### Why declarative?

Declarative specs are easier to understand as their give the whole grasp of what's going on or what we want to be done. Let the media server to handle the concrete steps, encapsulate and optimize their complexity.

The success of such approach is approved by [Kubernetes], [Docker Compose], [Ansible] and many other tools.

As alternative, providing an imperative Control API will increase the usage complexity, as will make user to deal with correct initialization order, to maintain "the whole state picture" on his side, and to deal with unexpected races which can be naturally handled by media server inside itself.


### Why dynamic media data pipeline declaration?

Dynamic elements declaration can be [easily used in static context][static-declarations], while vice versa is not true. Our experience has showed that static declarations are often not enough for business requirements, so using only them can be tricky and inconvenient in some scenarios. Let the developer choose what lifecycle he wants.


### Why `Element`-based?

Creating `Element`-centric system allows to extend the system with any kinds of `Element`s, so remove any restrictions of what can be implemented.

The alternative is to provide the concrete scenarios configurations (like WebRTC room, VOD files server, etc), but this will limit the usage of media server and disallow fine-grained media data pipeline configuration.

This part of design is heavily inspired by [GStreamer].


### Why different kinds of APIs?

While this is not mandatory to implement all the kinds instantly, this RFC shows that media server Control API can be exposed in almost any protocols/technologies. This will allow the media server to integrate naturally in various topologies and platforms.

The start should be a [gRPC] interface, and the HTTP REST interface, because [gRPC] is easy-to-go (just generate the code from the spec), while REST HTTP is more widely supported and more convenient for debugging.


### Why callbacks?

Callback is the easiest way to integrate with third-party applications. The alternative to this can be allowing media server users to write and enable control plugins, which is better in terms of performance, but far more complex to deal with.

The performance penalty may be mitigated with providing streaming-based callbacks, or callbacks which write events directly into some storage.


### Why auto-removing rules?

Dynamic declaration makes developer to worry of when and how declarations should be removed after they are not required anymore. Auto-removing tries to simplify developer's work by handling these questions automatically, which is what-we-want in most cases, and so to avoid undesired resource leaks. This is very similar to the benefits we have from [GC]/[RAII] in programming languages.




## Prior art
[prior-art]: #prior-art

Usually, media server provides some static configuration and ability to react on its internal events. While this is convenient in many cases, sometimes (for some kinds of workloads/topologies) it's hard (or inconvenient) to deal with.

This RFC design, however, tries to accomplish a larger aim, and in many parts evolves from our experience, yet inspired by the following:
- [GStreamer pipelines][1], which allow to *build any desired media data pipeline*.
- [Kubernetes Objects][3], which allow to *express resources and topology in a convenient declarative way*.
- [OpenVidu Server API][2], which allows *dynamic creation/deletion of media data pipelines*.




## Unresolved questions
[unresolved-questions]: #unresolved-questions

The concrete set of media `Element`s is an open question. However, this question can be truly answered only during implementation and beta experience, as it's hard to predict all the edge cases and nuances now.




## Future possibilities
[future-possibilities]: #future-possibilities


### Flexible integration

The callback part of Control API can be extended to support almost any kind of interaction: [WebSocket] connections (`ws://`), [gRPC] streaming (`stream+grpc://`), [Apache Kafka] topics (`kafka://`), and so on. This will make an integration easy and suitable for various kinds of workloads and topologies.


### Kubernetes Operator

Declarative Control API allows media server to act as [Kubernetes Operator], so consume declarations via [Custom Resource Definitions in Kubernetes API][4], manage its state not internally but in [Kubernetes resources storage][7], and report its events as first-class [Kubernetes events][8]. This gives an out-of-the-box [cloud native][5] support for media server, which can be very convenient for [Kubernetes] users and platforms like [GKE] and/or [Amazon EKS].


### Static declarations
[static-declarations]: #static-declarations

Static media pipeline creation is still possible with a concept similar to [Kubernetes static pods][6]. Some kinds of media pipelines can be declared statically in a form of manifest files consumed by media server on start and exist while the media server is up. This feature will be very convenient for those who wants "just serve a bunch of files".


### Media elements versioning

Similarly to [Kubernetes] the Control API can provide versions for its media elements, so allow a convenient media elements evolving with deprecation process and without introducing breaking changes. 





[Amazon EKS]: https://aws.amazon.com/eks
[Ansible]: https://www.ansible.com
[Apache Kafka]: https://kafka.apache.org
[Docker Compose]: https://docs.docker.com/compose
[GC]: https://en.wikipedia.org/wiki/Garbage_collection_(computer_science)
[GKE]: https://cloud.google.com/kubernetes-engine
[gRPC]: https://grpc.io
[GStreamer]: https://gstreamer.freedesktop.org
[HLS]: https://en.wikipedia.org/wiki/HTTP_Live_Streaming
[Kubernetes]: https://kubernetes.io
[Kubernetes Operator]: https://coreos.com/operators
[MessagePack]: https://msgpack.org
[OpenVidu]: https://openvidu.io
[RAII]: https://en.wikipedia.org/wiki/Resource_acquisition_is_initialization
[RFC 3339]: https://www.ietf.org/rfc/rfc3339.txt
[Swagger]: https://swagger.io
[WebRTC]: https://webrtc.org
[WebSocket]: https://en.wikipedia.org/wiki/WebSocket

[1]: https://gstreamer.freedesktop.org/documentation/application-development/introduction/basics.html
[2]: https://openvidu.io/docs/reference-docs/REST-API
[3]: https://kubernetes.io/docs/concepts/overview/working-with-objects/kubernetes-objects
[4]: https://kubernetes.io/docs/concepts/extend-kubernetes/api-extension/custom-resources
[5]: https://container-solutions.com/what-is-cloud-native
[6]: https://kubernetes.io/docs/tasks/administer-cluster/static-pod
[7]: https://kubernetes.io/docs/concepts/overview/components/#etcd
[8]: https://kubernetes.io/docs/tasks/debug-application-cluster/events-stackdriver
