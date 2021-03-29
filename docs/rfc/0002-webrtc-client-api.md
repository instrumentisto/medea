- Feature Name: `client_webrtc_api`
- Start Date: 2018-12-13
- RFC PR: [instrumentisto/medea#7](https://github.com/instrumentisto/medea/pull/7)
- Tracking Issue: [instrumentisto/medea#6](https://github.com/instrumentisto/medea/issues/6)




## Summary
[summary]: #summary

Formalize communication protocol between client (browser, mobile apps) and media server regarding [WebRTC] connection management.




## Motivation
[motivation]: #motivation

[WebRTC] allows P2P data exchange, but [WebRTC] as a protocol comes without signaling. At the minimum signalling protocol must provide ways to exchange Session Description data ([SDP Offer] / [SDP Answer]) and [ICE Candidate]s. But if you think about signalling protocol in terms of interaction with media server things become more complicated.

You will need to express ways to:
1. Provide [STUN]/[TURN] servers.
2. Exchange some low-level media metadata (resolution, codecs, media types).
3. Allow more sophisticated management of media tracks (updating video resolution on preview/fullscreen switches, passing multiple video tracks with different settings).
4. Pass some user metadata to hook business logic onto.
5. Build more complex connection graphs.
6. Dynamically cancel/begin media publishing/receiving.
7. Passing errors, [RTCStatsReport]s of underlying [RTCPeerConnection]s.
8. Cover both [P2P full mesh] and hub server ([SFU], [MCU]) scenarios.

The protocol must be versatile enough to cover all possible use cases.




## Guide-level explanation
[guide-level-explanation]: #guide-level-explanation


### What is `Client WebRTC API`? 

`Client WebRTC API` is a part of `Client API` responsible for [WebRTC] connection management. You can find `Client API` on the following approximate architecture design:
```
                                                                       .------------Server-----------.
                                                                       :     .-------------------.   :
                          .--------------------------------------------+-----o  Control Service  :   :
                          :                                            :     '--------o----------'   :
                          :                                            :              |              :
                          :                                            :        Control Api          :
.--------Client-----------+------------------------.                   :              |              :
:  .--------------------. :  .--------------------. :  .-Client-API--. :  .-----------o------------. :
:  :  User Application  o-'  :     Web Client     o-+--'             '-+--o      Media Server      : :
:  :                    :----:                    o-+--.             .-+--o                        : :
:  '--------------------'    '--------------------' :  '----Media----' :  '------------------------' :
'---------------------------------------------------'                  '-----------------------------'
```

So, how it works from `Media Server` point of view:
1. `Control Service` configures media room via [`Control API`][Control API].  
2. `Media Server` provides all necessary information (URLs + credentials) for all room members.
3. `User Application` passes credentials and other necessary stuff (like `<video>` elements) to `Web Client`.
4. ...and voilà!


### Transport considerations

Although, signalling can be implemented on top of any transport, [WebSocket] suits the most since it provides small overhead reliable duplex connection, and is widely adopted and supported.


### WebSocket considerations

Existing best practices are recommended for final implementation:
1. Message level `ping`/`pong`, since it is the most reliable way to detect connection loss (protocol level [WebSocket] `ping`/`pong` may disfunct due to browser implementation and is not exposed to `Web Client`).
2. Reconnects, since [RTCPeerConnection] always outlives [WebSocket] connection in cases of network issues, and both parts should know when to dispose related resources.
3. Using custom Close Frame Status Codes, to implement reliable send-and-close.


### Signalling Protocol considerations
[signalling-protocol-considerations]: #signalling-protocol-considerations

One of the main goals, is to make `Web Client` integration as easy as possible. This means less interaction between `User Application` and `Web Client`, more interaction between `Web Client` and `Media Server`, and quite verbose [`Control API`][Control API] design.

Having in mind, that `Media Server` already has user connection graph received from `Control Service` by the moment user connects, it is possible to establish all required connections without bothering `User Application`. Basically, connection establishment may not depend on interaction with `User Application` at all.

On the other hand, some use cases require more manual control over media exchange process. For example:
1. User wants to receive lower resolution video.
2. User wants to stop sending media to specific user.
3. And then start sending media again.
4. Mute or unmute.

So, possible API designs can be divided in two categories:
1. Preconfigured: where everything works out-of-the-box and almost no interaction between `User Application` and `Web Client` required.
2. Dynamic: when `User Application` needs to express complex use cases and change the topology dynamically.

Current RFC offers combining both ways: everything will be configured automagically, but dynamic API is always there if you need it.

#### Messaging

All [WebSocket] messages sent by `Media Server` are called `Event`s. `Event` means a fact that already has happened, so `Web Client` cannot reject `Event` in any way (you cannot reject the happened past), it can only adopt itself to the received `Event`s. So, `Media Server` just notifies `Web Client` about happened facts and it reacts on them to reach the proper state. This also emphasizes the indisputable authority of the `Media Server`.

All [WebSocket] messages sent by `Web Client` are called `Command`s. `Command` is basically a request/desire/intention of `Web Client` to change the state on `Media Server`.




## Reference-level explanation
[reference-level-explanation]: #reference-level-explanation


### Data model and primitives

```
   .-------------------------Member---------------------------.
   : .-----------Peer----------.  .------------Peer---------. :
   : : .--Track--. .--Track--. :  : .--Track--. .--Track--. : :
   : : :  video  : :  audio  : :  : :  video  : :  audio  : : :
   : : :         : :         : :  : :         : :         : : :
   : : '----o----' '---------' :  : '---------' '----o----' : :
   : '------|------------------'  '------------------|------' :
   '--------V----------------------------------------Λ--------'
            :                                        :
            :                                        Λ
            :------->------>-------.                 '---.
            :                      :                     :
            V                      :                     Λ
            :                      :                     :
   .--------V--------.    .--------V--------.   .--------Λ--------.
   : .------|------. :    : .------|------. :   : .------|------. :
   : : .----o----. : :    : : .----o----. : :   : : .----o----. : :
   : : :  video  : : :    : : :  video  : : :   : : :  audio  : : :
   : : :         : : :    : : :         : : :   : : :         : : :
   : : '--Track--' : :    : : '--Track--' : :   : : '--Track--' : :
   : '-----Peer----' :    : '-----Peer----' :   : '-----Peer----' :
   '------Member-----'    '------Member-----'   '------Member-----'
```

#### Member

Just a way to group `Peers` and provide `User Application` with some user metadata. `Member` can have 0-N `Peer`s.

```rust
struct Member {
    member_id: String,
    peers: Vec<u64>,
}
```

#### Peer

[RTCPeerConnection] representation. `Peer` can have 1-N `Track`s.

```rust
struct Peer {
    peer_id: u64,
    tracks: Vec<Track>,
}
```

#### Track

[MediaStreamTrack] representation.

```rust
struct Track {
    id: u64,
    media_type: TrackMediaType,
    direction: TrackDirection,
}

enum TrackDirection {
    Send {
      receivers: Vec<u64>,
      mid: Option<String>,
    },
    Recv {
      sender: u64,
      mid: Option<String>,
    },
}

enum TrackMediaType {
    Audio(AudioSettings),
    Video(VideoSettings),
}

struct AudioSettings {}

struct VideoSettings {}
```


### Events

[WebSocket] messages from `Media Server` to `Web Client`.

The naming for `Event` follows the convention `<entity><passive-verb>`, for example: `PeerCreated`, `PeerUpdated`, `PeersRemoved`.

The format of `Event` [WebSocket] message may be implemented as the following:
```rust
struct EventWebSocketMessage {
    event: String,
    data: EventData,
    meta: Option<EventMeta>,
}
```
Where:
- `event`: name of concrete `Event` (declared below);
- `data`: data provided by this `Event` (declared below);
- `meta`: optional metadata of the `Event` for debugging or tracing purposes (may be fully omitted).

#### 1. PeerCreated

```rust
struct PeerCreated {
    peer_id: PeerId,
    sdp_offer: Option<String>,
    tracks: Vec<Track>,
    ice_servers: Vec<IceServer>,
    force_relay: bool,
}
```

Related objects:
```rust
struct IceServer {
    urls: Vec<String>,
    username: String,
    credential: String,
}
```

`Media Server` notifies about necessity of [RTCPeerConnection] creation.

Params:
1. `peer_id`: created `Peer`'s ID.
2. `sdp_offer`: if `None`, client should create [SDP Offer] and pass it to the server; if `Some`, client should set it as remote description, then create [SDP Answer], set it as local description, and pass it to the server.
3. `tracks`: tracks of this `Peer`.
4. `ice_servers`: list of [ICE server]s that should be used to construct [RTCPeerConnection].
5. `force_relay`: if `true` then all media will be relayed through [TURN] server.

The most important part of `Peer` object is a list of `Track`s.
- All `TrackDirection::Send` `Track`s must be created according to their settings and added to the `Peer`. 
- If there is at least one `TrackDirection::Recv` `Track`, then created [RTCPeerConnection] must be ready to receive `Track`s (`recvonly`/`sendrecv` SDP). Currently, there are multiple ways to achieve this on client side and concrete implementation is not part of this RFC.

##### Examples

<details>
<summary>Create Audio+Video sendrecv Peer</summary>

```json
{
  "peer_id": 1,
  "tracks": [{
    "id": 1,
    "media_type": {
      "Audio": {}
    },
    "direction": {
      "Send": {
        "receivers": [2],
        "mid": null
      }
    }
  }, {
    "id": 2,
    "media_type": {
      "Video": {}
    },
    "direction": {
      "Send": {
        "receivers": [2],
        "mid": null
      }
    }
  }, {
    "id": 3,
    "media_type": {
      "Audio": {}
    },
    "direction": {
      "Recv": {
        "sender": 2,
        "mid": null
      }
    }
  }, {
    "id": 4,
    "media_type": {
      "Video": {}
    },
    "direction": {
      "Recv": {
        "sender": 2,
        "mid": null
      }
    }
  }],
  "sdp_offer": null,
  "ice_servers": [{
    "urls": [
      "turn:turnserver.com:3478",
      "turn:turnserver.com:3478?transport=tcp"
    ],
    "username": "turn_user",
    "credential": "turn_credential"
  }],
  "force_relay": false
}
```

`Web Client` is expected to:
1. Create [RTCPeerConnection] with provided [ICE server]s and associate it with given `peer_id`.
2. Initialize `Audio` and `Video` `Track`s without any additional settings.
3. Add newly created `Track`s to [RTCPeerConnection].
4. Generate `sendrecv` [SDP Offer].
5. Set offer as `Peer`'s local description.
6. Answer with `MakeSdpOffer` command containing generated [SDP Offer].
7. Expect remote [SDP Answer] to set it as remote description.

After negotiation is done and media starts flowing, `Web Client` might receive notification that his media is being sent to `Peer { peer_id = 2 }` and he is receiving media from `Peer { peer_id = 2 }`.
</details>

<details>
<summary>Create Audio send to SFU Peer</summary>

```json
{
  "peer_id": 1,
  "tracks": [{
    "id": 1,
    "media_type":{
      "Audio":{}
    },
    "direction": {
      "Send": {
        "receivers": [],
        "mid": null
      }
    }
  }],
  "sdp_offer": "server_user1_recvonly_offer",
  "ice_servers": [{
    "urls": [
      "turn:turnserver.com:3478",
      "turn:turnserver.com:3478?transport=tcp"
    ],
    "username": "turn_user",
    "credential": "turn_credential"
  }],
  "force_relay": false
}
```

`Web Client` is expected to:
1. Create [RTCPeerConnection] with provided [ICE server]s and associate it with given `peer_id`.
2. Initialize `Audio` `Track` without any additional settings.
3. Add newly created `Track` to [RTCPeerConnection].
4. Set provided [SDP Offer] as `Peer`'s remote description.
5. Generate `sendonly` [SDP Answer].
6. Set created [SDP Answer] as local description.
7. Answer with `MakeSdpAnswer` command containing generated [SDP Answer]. 

After negotiation is done and media starts flowing, `Web Client` might receive notification that his media is being sent to `Media Server`.
</details>

#### 2. PeersRemoved

```rust
struct PeersRemoved {
    peer_ids: Vec<u64>,
}
```

`Media Server` notifies about necessity to dispose (close and remove) specified `Peer`s.

##### Examples

<details>
<summary>Server tells Web Client to dispose specified Peers</summary>

```json
{
  "peer_ids": [1, 2, 3]
}
```
</details>

#### 3. PeerUpdated

```rust
pub enum NegotiationRole {
    Offerer,
    Answerer(String),
}

enum PeerUpdate {
    Added(Track),
    Updated(TrackPatch),
    IceRestart,
}

struct PeerUpdated {
    peer_id: u64,
    updates: Vec<PeerUpdate>,
    negotiation_role: Option<NegotiationRole>,
}
```

`Media Server` notifies about necessity to update specified `Peer`.

It can be used to:
1. Add new `Track`.
2. Update existing `Track` settings (e.g. change to lower video resolution, mute audio).
3. Update `send` `Track` receivers list (add/remove).
4. Perform ICE restart.

##### Examples 

<details>
<summary>If Peer exists on Web Client's end</summary>

```json
{
  "peer_id": 1,
  "tracks": [{
    "id": 1,
    "media_type": {
      "Audio": {}
    },
    "direction": {
      "Send": {
        "receivers": []
      }
    }
  }, {
    "id": 2,
    "media_type": {
      "Video": {}
    },
    "direction": {
      "Send": {
        "receivers": []
      }
    }
  }]
}
```

Means that media is being published to `Media Server` but has no actual receivers.
</details>

<details>
<summary>Media Server notifies Web Client that video is being received by other Peer</summary>

```json
{
  "peer_id": 1,
  "tracks": [{
    "id": 1,
    "media_type": {
      "Audio": {}
    },
    "direction": {
      "Send": {
        "receivers": [2]
      }
    }
  }, {
    "id": 2,
    "media_type": {
      "Video": {}
    },
    "direction": {
      "Send": {
        "receivers": [2]
      }
    }
  }]
}
```
</details>

#### 4. TracksRemoved

```rust
struct TracksRemoved {
    peer_id: u64,
    tracks: Vec<u64>,
}
```

`Media Server` notifies about necessity to dispose (close and remove) specified `Track`s.

##### Examples

<details>
<summary>Media Server tells Web Client to dispose specified Tracks</summary>

```json
{
  "peer_id": 1,
  "tracks": [1, 2]
}
```
</details>

#### 5. SdpOfferMade

```rust
struct SdpOfferMade {
    peer_id: u64,
    sdp_offer: String,
}
```

`Media Server` notifies about necessity to apply specified [SDP Offer] to `Web Client`'s [RTCPeerConnection].

This event is sent during SDP negotiation/re-negotiation. `Web Client` is expected answer with `MakeSdpAnswer` command.

##### Examples

<details>
<summary>Media Server sends SDP Offer to Peer</summary>

```json
{
  "peer_id": 1,
  "sdp_offer": "sdp_offer_body"
}
```
</details>

#### 6. SdpAnswerMade

```rust
struct SdpAnswerMade {
    peer_id: u64,
    sdp_answer: String,
}
```

`Media Server` notifies about necessity to apply specified [SDP Answer] to `Web Client`'s [RTCPeerConnection].

This event is sent during SDP negotiation/re-negotiation.

##### Examples

<details>
<summary>Media Server sends SDP Answer to Peer</summary>

```json
{
  "peer_id": 1,
  "sdp_offer": "sdp_answer_body"
}
```
</details>

#### 7. IceCandidateDiscovered

```rust
struct IceCandidateDiscovered {
    peer_id: u64,
    candidate: IceCandidate,
}
```

Related objects:
```rust
struct IceCandidate {
    candidate: String,
    sdp_m_line_index: Option<u16>,
    sdp_mid: Option<String>,
}
```

`Media Server` notifies about necessity to apply [ICE Candidate] to `Web Client`'s [RTCPeerConnection].

This event is sent during ICE negotiation/re-negotiation.

##### Examples

<details>
<summary>Media Server sends ICE Candidate to Peer</summary>

```json
{
  "peer_id": 1,
  "candidate": "ice_cadidate"
}
```
</details>

#### 8. RemotePeersUpdated

```rust
struct RemotePeersUpdated {
    peers: Vec<RemotePeer>
}
```

Related objects:
```rust
struct RemotePeer {
    peer_id: Option<u64>,
    member_id: Option<String>,
    can_rx: Option<RemotePeerTrackType>,
    can_tx: Option<RemotePeerTrackType>,
}

enum RemotePeerTrackType {
    Audio {
        audio_settings: Option<AudioSettings>,
    },
    Video {
        video_settings: Option<VideoSettings>,
    },
    AudioVideo {
        audio_settings: Option<AudioSettings>,
        video_settings: Option<VideoSettings>,
    },
}
```

`Media Server` notifies about any remote `Peer`s that `Web Client` can connect to.

This is a key event when talking about dynamic API mentioned in [Signalling Protocol considerations][signalling-protocol-considerations]. Any `Web Client`'s commands to subscribe/publish will be based on data provided by this event.

Params:
1. `peer_id`: if `Some`, then represents specific remote `Peer` associated with some `Member`; if `None`, then represents `Media Server`'s [RTCPeerConnection].
2. `member_id`: if `Some`, then represents specific remote `Member`; if `None`, then represents `Media Server`'s [RTCPeerConnection].
3. `can_rx`: if `Some` then `Web Client` can subscribe to specified media.
4. `can_tx`: if `Some` then `Web Client` can publish specified media to remote `Peer`.

##### Examples

<details>
<summary>Notify Web Client that it is possible to subscribe to another Member's Video and Audio Tracks</summary>

```json
{
  "peers": [{
    "peer_id": 2,
    "member_id": "User2",
    "can_rx": {
      "AudioVideo": {
        "audio_settings": {},
        "video_settings": {}
      }
    },
    "can_tx": null
  }]
}
```
</details>

<details>
<summary>Notify Web Client that it is possible to publish Audio to specified Peers</summary>

```json
{
  "peers": [{
    "peer_id": 2,
    "member_id": "User2",
    "can_rx": null,
    "can_tx": {
      "Audio": {
        "audio_settings": {}
      }
    }
  }, {
    "peer_id": 3,
    "member_id": "User3",
    "can_rx": null,
    "can_tx": {
      "Audio": {
        "audio_settings": {}
      }
    }
  }]
}
```
</details>

#### 9. MembersUpdated 

```rust
struct MembersUpdated {
    members: Vec<Member>
}
```

`Media Server` updates `Web Client`'s knowledge about `Peer`<=>`Member` associations.

It's recommended to cache `Peer` ID and `Member` ID relations in `Web Client`'s local state (for example, in two maps: `HashMap<peer_id, member_id>`, `HashMap<member_id, peer_id>`).

##### Examples

<details>
<summary>Media Server updates Web Client's knowledge about Peer<=>Member associations</summary>

```json
{
  "members": [{
    "member_id": "user_2",
    "peers": [1]
  }, {
    "member_id": "user_2",
    "peers": [2]
  }, {
    "member_id": "user_3",
    "peers": [3, 4]
  }]
}
```
</details>

#### 10. ConnectionQualityUpdated

```rust
pub enum ConnectionQualityScore {
    Poor = 1,
    Low = 2,
    Medium = 3,
    High = 4,
}

struct ConnectionQualityUpdated {
    partner_member_id: MemberId,
    quality_score: ConnectionQualityScore,
}
```

`Media Server` notifies `Web Client` about connection quality score update.


### Commands

[WebSocket] message from `Web Client` to `Media Server`.

The naming for `Command` follows the convention `<infinitive-verb><entity>`, for example: `ApplyTracks`, `MakeSdpOffer`, `MakeSdpAnswer`.

The format of `Command` [WebSocket] message may be implemented as the following:
```rust
struct CommandWebSocketMessage {
    command: String,
    data: CommandData,
    meta: Option<CommandMeta>,
}
```
Where:
- `command`: name of concrete `Command` (declared below);
- `data`: data provided by this `Command` (declared below);
- `meta`: optional metadata of the `Command` for debugging or tracing purposes (may be fully omitted).

#### 1. RemovePeers

```rust
struct RemovePeers {
    peer_ids: Vec<u64>,
}
```

`Web Client` asks permission to dispose (close and remove) specified `Peer`s. `Media Server` gives permission by sending `PeersRemoved` event.

Probably, `Media Server` will always give this permission on any `Web Client`'s command. This kind of messages flow will allow `Media Server` to do any command-related stuff that `Media Server` needs to do, and distinguish between abnormal and normal events.

##### Examples

<details>
<summary>Web Client asks permission to dispose specified Peers</summary>

```json
{
  "peer_ids": [1, 2, 3]
}
```
</details>

#### 2. ApplyTracks

```rust
struct ApplyTracks {
    peer_id: u64,
    tracks: Vec<Track>,
}
```

`Web Client` asks permission to update `Track`s in specified `Peer`. `Media Server` gives permission by sending `PeerUpdated` event.

It can be used to express `Web Client`'s intentions to:
1. Update existing `Track` settings.
2. Cancel sending media to specific receiver.

##### Examples 

<details>
<summary>If Peer exists on Web Client's end</summary>

```json
{
  "peer_id": 1,
  "tracks": [{
    "id": 1,
    "media_type": {
      "Audio": {}
    },
    "direction": {
      "Send": {
        "receivers": [2]
      }
    }
  }, {
    "id": 2,
    "media_type": {
      "Video": {}
    },
    "direction": {
      "Send": {
        "receivers": [2]
      }
    }
  }]
}
```

Means that media is being published to `Media Server` and relayed to `Peer {peer_id = 2}`.
</details>

<details>
<summary>Web Client wants to unsubscribe Peer from specified Tracks</summary>

```json
{
  "peer_id": 1,
  "tracks": [{
    "id": 1,
    "media_type": {
      "Audio": {}
    },
    "direction": {
      "Send": {
        "receivers": []
      }
    }
  }, {
    "id": 2,
    "media_type": {
      "Video": {}
    },
    "direction": {
      "Send": {
        "receivers": []
      }
    }
  }]
}
```
</details>

#### 3. RemoveTracks

```rust
struct RemoveTracks {
    peer_id: u64,
    tracks: Vec<u64>,
}
```

`Web Client` asks permission to dispose (close and remove) specified `Track`s. `Media Server` gives permission by sending `TracksRemoved` event.

##### Examples

<details>
<summary>Web Client asks permission to dispose specified Tracks</summary>

```json
{
  "peer_id": 1,
  "tracks": [1, 2]
}
```
</details>

#### 4. MakeSdpOffer

```rust
struct MakeSdpOffer {
    peer_id: u64,
    sdp_offer: String,
    mids: HashMap<TrackId, String>,
}
```

`Web Client` sends [SDP Offer] to one if its `Peer`s. `mids` section specifies for each `Track`  its transceivers [media descriptions](https://tools.ietf.org/html/rfc4566#section-5.14). 

`Web Client` can send it:
1. As reaction to `PeerCreated {sdp_offer: None}` event.
2. As reaction to `PeerUpdated` event if update requires SDP re-negotiation.

##### Examples

<details>
<summary>Web Client sends SDP Offer to some Peer</summary>

```json
{
  "peer_id": 1,
  "sdp_offer": "sdp_offer_body"
}
```
</details>

#### 5. MakeSdpAnswer

```rust
struct MakeSdpAnswer {
    peer_id: u64,
    sdp_answer: String,
}
```

`Web Client` sends [SDP Answer] to one if its `Peer`s.

`Web Client` can send it:
1. As reaction to `PeerCreated {sdp_offer: Some}` event.
2. As reaction to `SdpOfferMade` event.

##### Examples

<details>
<summary>Web Client sends SDP Answer to some Peer</summary>

```json
{
  "peer_id": 1,
  "sdp_offer": "sdp_answer_body"
}
```
</details>

#### 6. SetIceCandidate

```rust
struct SetIceCandidate {
    peer_id: u64,
    candidate: IceCandidate,
}
```

Related objects:
```rust
struct IceCandidate {
    candidate: String,
    sdp_m_line_index: Option<u16>,
    sdp_mid: Option<String>,
}
```

`Web Client` sends [ICE Candidate] discovered by underlying [RTCPeerConnection] for one of his `Peer`s.

##### Examples

<details>
<summary>Web Client sends ICE Candidate for some Peer</summary>

```json
{
  "peer_id": 1,
  "candidate": "ice_cadidate"
}
```
</details>

#### 7. RequestRemoteTracks

```rust
struct RequestRemoteTracks {
    peer_id: Option<u64>,
    remote_peer_id: Option<u64>,
    rx: Option<RemotePeerTrackType>,
    tx: Option<RemotePeerTrackType>,
}
```

Related objects:
```rust
enum RemotePeerTrackType {
    Audio {
        audio_settings: Option<AudioSettings>,
    },
    Video {
        video_settings: Option<VideoSettings>,
    },
    AudioVideo {
        audio_settings: Option<AudioSettings>,
        video_settings: Option<VideoSettings>,
    },
}
```

`Web Client` asks permission to send or receive media to/from specified remote `Peer`. `Media Server` may gives permission by sending `PeerUpdated` event.

Params:
1. `peer_id`: if `Some`, then `Web Client` wants to connect specified local `Peer` to remote one; if `None`, then `Media Server`
decides which of `Web Client`'s `Peer`s will be connected.
2. `remote_peer_id`: if `Some`, then represents specific remote `Peer`; if `None`, then represents `Media Server`'s [RTCPeerConnection], but only for [SFU].
3. `rx`: if `Some` then `Web Client` requests to subscribe to specified media.
4. `tx`: if `Some` then `Web Client` requests to publish specified media to remote `Peer`.

##### Examples

<details>
<summary>Web Client requests to subscribe to remote Peer audio and video</summary>

```json
{
  "peer_id": 1,
  "remote_peer_id": 2,
  "rx": null,
  "tx": {
    "AudioVideo": {
      "audio_settings": {},
      "video_settings": {}
    }
  }
}
```
</details>

<details>
<summary>Web Client requests to publish to Media Server's RTCPeerConnection</summary>

```json
{
  "peer_id": 1,
  "remote_peer_id": null,
  "rx": null,
  "tx": {
    "AudioVideo": {
      "audio_settings": {},
      "video_settings": {}
    }
  }
}
```
</details>

#### 9. GetMembers

```rust
struct GetMembers {
    peer_ids: Vec<u64>,
}
```

`Web Client` asks IDs of present `Member`s for specified `Peer`s. `Media Server` answers with `MembersUpdated` event.

##### Examples

<details>
<summary>Web Client request Members which own specified Peers</summary>

```json
{
  "peer_ids": [
    2, 3, 4
  ]
}
```
</details>

#### 9. AddPeerConnectionMetrics

`Web Client` sends [RTCPeerConnection] metrics.

```rust
struct AddPeerConnectionMetrics {
    peer_id: u64,
    metrics: PeerMetrics,
}
```

Related objects:
```rust
pub enum PeerMetrics {
    IceConnectionState(IceConnectionState),
    PeerConnectionState(PeerConnectionState),
    RtcStats(Vec<RtcStat>),
}

pub enum IceConnectionState {
    New,
    Checking,
    Connected,
    Completed,
    Failed,
    Disconnected,
    Closed,
}

pub enum PeerConnectionState {
    Closed,
    Failed,
    Disconnected,
    New,
    Connecting,
    Connected,
}
```

`RtcStat` object represents [RTCStats](https://www.w3.org/TR/webrtc/#dom-rtcstats) dictionary from the WebRTC specification.

All types of the `RtcStat` object you can find [here](https://www.w3.org/TR/webrtc-stats/#rtcstatstype-str*).

`Web Client` should send only updated `RtcStat`s in the `PeerMetrics::RtcStats` metric.

Metrics list will be extended as needed.

#### 10. UpdateTracks

`Web Client` asks permission to update `Track`s in specified `Peer`. `Media Server` gives permission by sending `Event::PeerUpdated`.

```rust
struct UpdateTracks {
    peer_id: PeerId,
    tracks_patches: Vec<TrackPatch>,
}

struct TrackPatch {
    pub id: TrackId,
    pub is_muted: Option<bool>,
}
```


### Extended examples

<details>
<summary>1 <=> 1 P2P with unpublish and republish</summary>

```
.----user1----.    .->-->-->--. .----user2----.
:             o(1)=:          :=o(2)          :
'-------------'    '-<--<--<--' '-------------'
```

1. `Media Server` sends `PeerCreated` event to `user1`:

    ```json
    {
      "event": "PeerCreated",
      "data": {
        "peer_id": 1,
        "tracks": [{
          "id": 1,
          "media_type": {
            "Audio": {}
          },
          "direction": {
            "Send": {
              "receivers": [2],
              "mid": null
            }
          }
        }, {
          "id": 2,
          "media_type": {
            "Video": {}
          },
          "direction": {
            "Send": {
              "receivers": [2],
              "mid": null
            }
          }
        }, {
          "id": 3,
          "media_type": {
            "Audio": {}
          },
          "direction": {
            "Recv": {
              "sender": 2,
              "mid": null
            }
          }
        }, {
          "id": 4,
          "media_type": {
            "Video": {}
          },
          "direction": {
            "Recv": {
              "sender": 2,
              "mid": null
            }
          }
        }],
        "sdp_offer": null,
        "ice_servers": [{
          "urls": [
            "turn:turnserver.com:3478",
            "turn:turnserver.com:3478?transport=tcp"
          ],
          "username": "turn_user",
          "credential": "turn_credential"
        }],
        "force_relay": false
      }
    }
    ```
 
2. `user1` answers with [SDP Offer]:

    ```json
    {
      "command": "MakeSdpOffer",
      "data": {
        "peer_id": 1,
        "sdp_offer": "user1_sendrecv_offer"
      },
      "mids": {
         "1": "0",
         "2": "1",
         "3": "2",
         "4": "3"
      }
    }
    ```
 
3. `Media Server` sends `PeerCreated` event with `user1`'s [SDP Offer] to `user2`:

    ```json
    {
      "event": "PeerCreated",
      "data": {
        "peer": {
          "peer_id": 2,
          "tracks": [{
            "id": 1,
            "media_type": {
              "Audio": {}
            },
            "direction": {
              "Recv": {
                "sender": 1,
                "mid": "0"
              }
            }
          }, {
            "id": 2,
            "media_type": {
              "Audio": {}
            },
            "direction": {
              "Recv": {
                "sender": 1,
                "mid": "1"
              }
            }
          }, {
            "id": 3,
            "media_type": {
              "Audio": {}
            },
            "direction": {
              "Send": {
                "receivers": [1],
                "mid": "2"
              }
            }
          }, {
            "id": 4,
            "media_type": {
              "Video": {}
            },
            "direction": {
              "Send": {
                "receivers": [1],
                "mid": "3"
              }
            }
          }]
        },
        "sdp_offer": "user1_sendrecv_offer",
        "ice_servers": [{
          "urls": [
            "turn:turnserver.com:3478",
            "turn:turnserver.com:3478?transport=tcp"
          ],
          "username": "turn_user",
          "credential": "turn_credential"
        }]
      }
    }
    ```

4. `user2` answers with [SDP Answer]:

    ```json
    {
      "command": "MakeSdpAnswer",
      "data": {
        "peer_id": 2,
        "sdp_answer": "user2_sendrecv_answer"
      }
    }
    ```

5. Both peers exchange discovered [ICE Candidate]s:

    1. `user1` => `Media Server`:

        ```json
        {
          "command": "SetIceCandidate",
          "data": {
            "peer_id": 1,
            "candidate": {
               "candidate": "user1_ice_candidate",
               "sdp_m_line_index": 0,
               "sdp_mid": "0"
             }
          }
        }
        ```

    2. `Media Server` => `user2`:

        ```json
        {
          "event": "IceCandidateDiscovered",
          "data": {
            "peer_id": 2,
            "candidate": {
               "candidate": "user1_ice_candidate",
               "sdp_m_line_index": 0,
               "sdp_mid": "0"
             }
          }
        }
        ```

    3. `user2` => `Media Server`:

        ```json
        {
          "command": "SetIceCandidate",
          "data": {
            "peer_id": 2,
            "candidate": {
               "candidate": "user2_ice_candidate",
               "sdp_m_line_index": 0,
               "sdp_mid": "0"
             }
          }
        }
        ```

    4. `Media Server` => `user1`:

        ```json
        {
          "event": "IceCandidateDiscovered",
          "data": {
            "peer_id": 1,
            "candidate": {
               "candidate": "user2_ice_candidate",
               "sdp_m_line_index": 0,
               "sdp_mid": "0"
             }
          }
        }
        ```

6. At this point connection is supposed to be established:

    ```
    .----user1----.    .->-->-->--. .----user2----.
    :             o(1)=:          :=o(2)          :
    '-------------'    '-<--<--<--' '-------------'
    ```
    
7. `user1` wants to unpublish his `Track`s, so he sends `RemoveTracks` command to `Media Server`:

    ```json
    {
      "command": "RemoveTracks",
      "data": {
        "peer_id": 1,
        "tracks": [1, 2]
      }
    }
    ```

8. `Media Server` updates `user2` `Track`s:

    ```json
    {
      "event": "TracksRemoved",
      "data": {
        "peer_id": 2,
        "tracks": [1, 2]
      }
    }
    ```

9. `Media Server` updates `user1` `Track`s:

    ```json
    {
      "event": "TracksRemoved",
      "data": {
        "peer_id": 1,
        "tracks": [1, 2]
      }
    }
    ```

10. `user1` initiates SDP re-negotiation: 

    1. `user1` sends `MakeSdpOffer` command.

    2. `Media Server` sends `SdpOfferMade` event to `user2`.

    3. `user2` sends `MakeSdpAnswer` command.

    4. `Media Server` sends `SdpAnswerMade` event to `user1`.

    ```
    .----user1----.         .----user2----.
    :             o(1)-<--<-o(2)          :
    '-------------'         '-------------'
    ```

11. `Media Server` notifies `user1` that he can publish to `user2`:

    ```json
    {
      "event": "RemotePeersUpdated",
      "data": {
        "peers": [{
          "peer_id": 2,
          "member_id": "user_2",
          "can_rx": null,
          "can_tx": {
            "AudioVideo": {
              "audio_settings": {},
              "video_settings": {}
            }
          }
        }]
      }
    }
    ```

12. `Media Server` notifies `user2` that he can subscribe to `user1`:

    ```json
    {
      "event": "RemotePeersUpdated",
      "data": {
        "peers": [{
          "peer_id": 1,
          "member_id": "user_1",
          "can_rx": {
            "AudioVideo": {
              "audio_settings": {},
              "video_settings": {}
            }
          },
          "can_tx": null
        }]
      }
    }
    ```

13. `user1` requests to publish to `user2`:

    ```json
    {
      "command": "RequestRemoteTracks",
      "data": {
        "peer_id": 1,
        "remote_peer_id": 2,
        "rx": null,
        "tx": {
          "AudioVideo": {
            "audio_settings": {},
            "video_settings": {}
          }
        }
      }
    }
    ```

14. `Media Server` updates `user2` `Track`s:

    ```json
    {
      "event": "PeerUpdated",
      "data": {
        "peer_id": 2,
        "tracks": [{
          "id": 1,
          "media_type": {
            "Audio": {}
          },
          "direction": {
            "Recv": {
              "sender": 1
            }
          }
        }, {
          "id": 2,
          "media_type": {
            "Audio": {}
          },
          "direction": {
            "Recv": {
              "sender": 1
            }
          }
        }]
      }
    }
    ```

15. `Media Server` updates `user1` `Track`s:

    ```json
    {
      "event": "PeerUpdated",
      "data": {
        "peer_id": 1,
        "tracks": [{
          "id": 1,
          "media_type": {
            "Audio": {}
          },
          "direction": {
            "Send": {
              "receivers": [2]
            }
          }
        }, {
          "id": 2,
          "media_type": {
            "Video": {}
          },
          "direction": {
            "Send": {
              "receivers": [2]
            }
          }
        }]
      }
    }
    ```

16. SDP re-negotiation: 

    1. `user1` sends `MakeSdpOffer` command.

    2. `Media Server` sends `SdpOfferMade` event to `user2`. 

    3. `user2` sends `MakeSdpAnswer` command.

    4. `Media Server` sends `SdpAnswerMade` event to `user1`.

    ```
    .----user1----.    .->-->-->--. .----user2----.
    :             o(1)=:          :=o(2)          :
    '-------------'    '-<--<--<--' '-------------'
    ```
</details>

<details>
<summary>1 => 2 SFU</summary>

```
                                                       .-------user2------.
                          .-------SFU-------.    .-->--o     pc_id = 2    :
.------user1------.       :       .---->----o-->-'     '------------------'
:    pc_id = 1    o-->-->-o--->---:         :
'-----------------'       :       '---->----o-->-.     .-------user3------.
                          '-----------------'    '-->--o     pc_id = 3    :
                                                       '------------------'
```

1. `Media Server` notifies `user1` to create `sendonly` `Peer` passing its [SDP Offer]:

    ```json
    {
      "event": "PeerCreated",
      "data": {
        "peer": {
          "peer_id": 1,
          "tracks": [{
            "id": 1,
            "media_type": {
              "Audio": {}
            },
            "direction": {
              "Send": {
                "receivers": [],
                "mid": "0"
              }
            }
          }, {
            "id": 2,
            "media_type": {
              "Video": {}
            },
            "direction": {
              "Send": {
                "receivers": [],
                "mid": "1"
              }
            }
          }]
        },
        "sdp_offer": "server_user1_recvonly_offer",
        "ice_servers": [{
          "urls": [
            "turn:turnserver.com:3478",
            "turn:turnserver.com:3478?transport=tcp"
          ],
          "username": "turn_user",
          "credential": "turn_credential"
        }]
      }
    }
    ```

2. `user1` creates `Peer` and answers with [SDP Answer]:

    ```json
    {
      "command": "MakeSdpAnswer",
      "data": {
        "peer_id": 1,
        "sdp_answer": "user_1_sendonly_answer"
      }
    }
    ```

3. `Media Server` and `user1` exchange their [ICE Candidate]s:

    1. `user1` => `Media Server`:

        ```json
        {
          "command": "SetIceCandidate",
          "data": {
            "peer_id": 1,
            "candidate": {
               "candidate": "user1_ice_candidate",
               "sdp_m_line_index": 0,
               "sdp_mid": "0"
             }
          }
        }
        ```

    2. `Media Server` => `user1`:

        ```json
        {
          "event": "IceCandidateDiscovered",
          "data": {
            "peer_id": 1,
            "candidate": {
               "candidate": "server_ice_candidate",
               "sdp_m_line_index": 0,
               "sdp_mid": "0"
             }
          }
        }
        ```

4. Connection is established:

    ```
                              .-------SFU-------.
    .------user1------.       :                 ;
    :    pc_id = 1    o-->-->-o                 :
    '-----------------'       :                 ;
                              '-----------------'
    ```

5. `Media Server` notifies `user2` to create `recvonly` `Peer` passing its [SDP Offer]:

    ```json
    {
      "event": "PeerCreated",
      "data": {
        "peer": {
          "peer_id": 2,
          "tracks": [{
            "id": 1,
            "media_type": {
              "Audio": {}
            },
            "direction": {
              "Recv": {
                "sender": 1,
                "mid": "0"
              }
            }
          }, {
            "id": 2,
            "media_type": {
              "Video": {}
            },
            "direction": {
              "Recv": {
                "sender": 1,
                "mid": "1"
              }
            }
          }]
        },
        "sdp_offer": "server_user2_sendonly_offer",
        "ice_servers": [{
          "urls": [
            "turn:turnserver.com:3478",
            "turn:turnserver.com:3478?transport=tcp"
          ],
          "username": "turn_user",
          "credential": "turn_credential"
        }]
      }
    }
    ```

6. `user2` answers with [SDP Answer]:

    ```json
    {
      "command": "MakeSdpAnswer",
      "data": {
        "peer_id": 2,
        "sdp_answer": "user_2_recvonly_answer"
      }
    }
    ```

7. `Media Server` and `user2` exchange their [ICE Candidate]s:

    1. `user2` => `Media Server`:

        ```json
        {
          "command": "SetIceCandidate",
          "data": {
            "peer_id": 2,
            "candidate": {
               "candidate": "user2_ice_candidate",
               "sdp_m_line_index": 0,
               "sdp_mid": "0"
             }
          }
        }
        ```

    2. `Media Server` => `user2`:

        ```json
        {
          "event": "IceCandidateDiscovered",
          "data": {
            "peer_id": 2,
            "candidate": {
               "candidate": "server_ice_candidate",
               "sdp_m_line_index": 0,
               "sdp_mid": "0"
             }
          }
        }
        ```

8. `user2` is connected to `Media Server`'s [RTCPeerConnection]:

    ```
                                                           .-------user2------.
                              .-------SFU-------.    .-->--o     pc_id = 2    :
    .------user1------.       :                 o-->-'     '------------------'
    :    pc_id = 1    o-->-->-o-                :
    '-----------------'       :                 :
                              '-----------------'
    ```

9. `Media Server` notifies `user1` that he has new subscriber:

    ```json
    {
      "event": "PeerUpdated",
      "data": {
        "peer_id": 1,
        "tracks": [{
          "id": 1,
          "media_type": {
            "Audio": {}
          },
          "direction": {
            "Send": {
              "receivers": [2]
            }
          }
        }, {
          "id": 2,
          "media_type": {
            "Video": {}
          },
          "direction": {
            "Send": {
              "receivers": [2]
            }
          }
        }]
      }
    }
    ```

10. `Media Server` sends `user1` `Peer {peer_id = 1}` media to `user2` `Peer {peer_id = 2}`:

    ```
                                                            .-------user2------.
                               .-------SFU-------.    .-->--o     pc_id = 2    :
     .------user1------.       :         .->-->--o-->-'     '------------------'
     :    pc_id = 1    o-->-->-o--->-->--'       :
     '-----------------'       :                 :
                               '-----------------'
    ```

11. `Media Server` notifies `user3` to create `recvonly` `Peer` passing its [SDP Offer]: 

    ```json
    {
      "event": "PeerCreated",
      "data": {
        "peer": {
          "peer_id": 3,
          "tracks": [{
            "id": 1,
            "media_type": {
              "Audio": {}
            },
            "direction": {
              "Recv": {
                "sender": 1,
                "mid": "0"
              }
            }
          }, {
            "id": 2,
            "media_type": {
              "Video": {}
            },
            "direction": {
              "Recv": {
                "sender": 1,
                "mid": "1"
              }
            }
          }]
        },
        "sdp_offer": "server_user3_sendonly_offer",
        "ice_servers": [{
          "urls": [
            "turn:turnserver.com:3478",
            "turn:turnserver.com:3478?transport=tcp"
          ],
          "username": "turn_user",
          "credential": "turn_credential"
        }]
      }
    }
    ```

12. `user2` answers with [SDP Answer]:

    ```json
    {
      "command": "MakeSdpAnswer",
      "data": {
        "peer_id": 3,
        "sdp_answer": "user_3_recvonly_answer"
      }
    }
    ```

13. `Media Server` and `user3` exchange their [ICE Candidate]s:

    1. `user3` => `Media Server`:

        ```json
        {
          "command": "SetIceCandidate",
          "data": {
            "peer_id": 3,
            "candidate": {
               "candidate": "user3_ice_candidate",
               "sdp_m_line_index": 0,
               "sdp_mid": "0"
             }
          }
        }
        ```

    2. `Media Server` => `user3`:

        ```json
        {
          "event": "IceCandidateDiscovered",
          "data": {
            "peer_id": 3,
            "candidate": {
               "candidate": "server_ice_candidate",
               "sdp_m_line_index": 0,
               "sdp_mid": "0"
             }
          }
        }
        ```

14. `user3` is connected to `Media Server`'s [RTCPeerConnection]:

    ```
                                                           .-------user2------.
                              .-------SFU-------.    .-->--o     pc_id = 2    :
    .------user1------.       :       .---->----o-->-'     '------------------'
    :    pc_id = 1    o-->-->-o--->---'         :
    '-----------------'       :                 o-->-.     .-------user3------.
                              '-----------------'    '-->--o     pc_id = 3    :
                                                           '------------------'
    ```

15. `Media Server` notifies `user1` that he has new subscriber:

    ```json
    {
      "event": "PeerUpdated",
      "data": {
        "peer_id": 1,
        "tracks": [{
          "id": 1,
          "media_type": {
            "Audio": {}
          },
          "direction": {
            "Send": {
              "receivers": [2, 3]
            }
          }
        }, {
          "id": 2,
          "media_type": {
            "Video": {}
          },
          "direction": {
            "Send": {
              "receivers": [2, 3]
            }
          }
        }]
      }
    }
    ```

16. `Media Server` sends `user1` `Peer {peer_id = 1}` media to `user3` `Peer {peer_id = 3}`:

    ```
                                                         .-------user2------.
                              .-------SFU-------.    .->-o     pc_id = 2    :
    .------user1------.       :       .---->----o-->-'   '------------------'
    :    pc_id = 1    o-->-->-o--->---:         :
    '-----------------'       :       '---->----o-->-.   .-------user3------.
                              '-----------------'    '->-o     pc_id = 3    :
                                                         '------------------'
    ```
</details>




## Drawbacks and alternatives
[drawbacks-and-alternatives]: #drawbacks-and-alternatives

This RFC design tries to be a "silver bullet": cover all possible use-cases and combine them into a single protocol. Such versatility increases complexity. Simplifications can be achieved by imposing some general constraints:
1. Divide current protocol in two separate protocols: one for [SFU] and one for [P2P full mesh].
2. Reject future possibilities of using 1 `Peer` for all inbound/outbound tracks.
3. Limit the number of outbound streams in a single `Peer` to 1.
4. Remove publishers acknowledgement of every receiver on each track.
5. Remove subscribers acknowledgement of every publisher that is not publishing at the moment.




## Unresolved questions and future possibilities
[unresolved-questions-and-future-possibilities]: #unresolved-questions-and-future-possibilities


### Data channels

[WebRTC] spec introduces [RTCDataChannel] - a bi-directional data channel between two peers which allows arbitrary data exchange. It is an amazing feature with huge potential, but, at this point it is quite useless for our use cases.

As the project grows, requirements will change, and we might consider adding data channels. Although, they are not mentioned in this protocol, only minor tweaks will be required to support them.


### Receiving tracks from multiple senders in a single peer connection

There are two general ways to manage `Web Client`'s [RTCPeerConnection]s when using [SFU] server:
1. Having only one pair of [RTCPeerConnection]s (one at `Web Client`'s end and one at `Media Server`) and pass all `send`/`recv` `Track`s through this connection.
2. Or having a separate [RTCPeerConnection] pair for each `Track` group.

First way is preferable since it allows to reduce resources usage on both ends. But `Track` management is very unclear in this case and [webrtcbin] module of [GStreamer] currently does not support dynamic addition/removal of streams and needs major updates to be able to do so.

Current protocol assumes that there will be separate [RTCPeerConnection] pair for each `Track` group. At the same time, it does not forbid having all the `Track`s in a single [RTCPeerConnection] pair, but it will require some minor changes to make this work.





[Control API]: https://github.com/instrumentisto/medea/blob/master/docs/rfc/0001-control-api.md
[GStreamer]: https://gstreamer.freedesktop.org
[ICE Candidate]: https://tools.ietf.org/html/rfc8445
[ICE server]: https://webrtcglossary.com/ice
[MCU]: https://webrtcglossary.com/mcu
[MediaStreamTrack]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
[P2P full mesh]: https://webrtcglossary.com/mesh
[RTCDataChannel]: https://www.w3.org/TR/webrtc/#rtcdatachannel
[RTCPeerConnection]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
[RTCStatsReport]: https://developer.mozilla.org/en-US/docs/Web/API/RTCStatsReport
[SDP Answer]: https://tools.ietf.org/html/rfc3264
[SDP Offer]: https://tools.ietf.org/html/rfc3264
[SFU]: https://webrtcglossary.com/sfu
[STUN]: https://tools.ietf.org/html/rfc3489
[TURN]: https://tools.ietf.org/html/rfc5766
[WebRTC]:https://www.w3.org/TR/webrtc
[webrtcbin]: https://gstreamer.freedesktop.org/data/doc/gstreamer/head/gst-plugins-bad/html/gst-plugins-bad-plugins-webrtcbin.html
[WebSocket]: https://en.wikipedia.org/wiki/WebSocket
