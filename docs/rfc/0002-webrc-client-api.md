- Feature Name: `client_webrtc_signalling_api`
- Start Date: 2018-12-13
- RFC PR: (leave this empty)
- Tracking Issue: (leave this empty)




## Summary
[summary]: #summary

Formalize communication protocol between client(browser, mobile apps) and media server regarding [WebRTC] connection 
management.

## Motivation
[motivation]: #motivation

[WebRTC] allows P2P data exchange, but [WebRTC] as a protocol comes without signaling. At a minimum signalling protocol 
must provide ways to exchange Session Description data([SDP Offer] / [SDP Answer]) and [ICE Candidate]. But if you think about 
signalling protocol in terms of interaction with media server things are becoming more complicated.

You will need to express ways to:
1. Provide STUN/TURN servers.
2. Exchange some low-level media metadata(resolution, codecs, media types).
3. Allow more sophisticated track management(updating video resolution on preview/fullscreen switches, passing multiple 
video tracks with different settings).
4. Pass some user metadata to hook business logic on.
5. Build more complex connection graphs.
6. Dynamically cancel/begin media publishing/receiving.
7. Passing errors, connection stats messages.
8. Cover both P2P mesh and SFU scenarios.

The protocol must be versatile enough to cover all possible use cases.

## Guide-level explanation
[guide-level-explanation]: #guide-level-explanation

### What is `WebRTC Client API`? 

It is a part of `Client API` responsible for [WebRTC] connection management. You can find `Client API` on approximate 
architecture design. 

```                                                   
                                                                       .------------Server-----------.
                                                                       :     .-------------------.   :
                          .--------------------------------------------+-----o  Control Service  :   :
                          :                                            :     '--------o----------'   :
                          :                                            :              |              :
                          :                                            :        Control Api          :
.--------Client-----------+------------------------.                   :              |              :
:  .--------------------. :  .--------------------. :  .-Client-API--. :  .-----------o------------. :
:  :  User Application  o-'  :  Medea Web Client  o-+--'             '-+--o   Medea Media Server   : :
:  :                    :----:                    o-+--.             .-+--o                        : :
:  '--------------------'    '--------------------' :  '----Media----' :  '------------------------' :
'---------------------------------------------------'                  '-----------------------------'
                           
```

So, how it works from `Medea Media Server` point of view:
1. `Control Service` configures media room via `Control API`.  
2. `Medea Media Server` provides all necessary information (urls+credentials) for all room members.
3. `User Application` passes credentials and other necessary stuff (like `<video>` elements) to `Medea Web Client`.
4. And voila!

### Transport considerations

Although, signalling can be implemented on top of any transport, WebSocket is suited the most since it provides small 
overhead reliable duplex connection. Widely used and supported.

### Protocol considerations

Existing best practices are recommended:
1. Message level ping-pongs.
2. Reconnects.
3. Transactions.
4. Using custom Close Frame Status Codes.

Transactions:

Each message is represented as:

```rust
struct WsMessage<T> {
    id: i64,
    payload: Option<Result<Payload<T>, Error>>,
}

struct Payload<T> {
    method: String,
    params: T,
}
```

Each message requires answer, answer can carry payload(e.g. answering with [SDP Answer] to [SDP Offer]), error, or nothing, 
which just means that message reached destination and was processed. 
 
### Signalling Protocol considerations

One of the main goals, is to make `Medea Web Client` integration as easy as possible. This means less interaction 
between `User Application` and `Medea Web Client` and more interaction between `Medea Web Client` and `Medea Media Server`, 
quite verbose `Control Api` design.

Having in mind, that `Medea Media Server` already has user connection graph received from `Control Service` by the 
moment user connects, it is possible to establish all required connections without bothering `User Application`. 
Basically connection establishment should not depend on interaction with `User Application`.

On the other hand, some use cases require more manual control over media exchange process. For example:
1. User wants to receive lower resolution video.
2. User wants to stop sending media to specific user.
3. And then start sending media again.
4. Mute/unmute.

So API can be divided in two categories:
1. Preconfigured: where everything works from the box and almost no interaction between `User Application` and 
`Medea Web Client` required.
2. Dynamic: when `User Application` needs to express complex use cases.

Current RFC offers combining both ways: everything will be configured automatically, but dynamic API is always there if 
you need it.

## Reference-level explanation
[reference-level-explanation]: #reference-level-explanation

### Data model

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

Just a way to group `Peers` and provide `User Application` with some users meta data. `Member` can have 0-N `Peers`.

```rust
struct Member {
    member_id: String,
    peers: Vec<u64>,
}
```

#### Peer

[RTCPeerConnection] representation. `Peer` can have 1-N `Tracks`.

```rust
struct Peer {
    peer_id: u64,
    p2p: bool,
    tracks: Vec<Track>,
}
```

#### Track

Somewhat [MediaStreamTrack] representation.

```rust
struct Track {
    id: u64,
    media_type: TrackMediaType,
    direction: TrackDirection,
}

enum TrackDirection {
    Send(Vec<u64>),     // receiver peers
    Recv(u64),          // sender peer
}

enum TrackMediaType {
    Audio(AudioSettings),
    Video(VideoSettings),
}

struct AudioSettings {}

struct VideoSettings {}
```

`P2P` flag implies some logic on `TrackDirection::Send` tracks:
1. `P2P` send tracks always have only one receiver.
2. Non `P2P` send tracks can have 0-N receivers. 0 - if media is transmitted to server, but have no actual user receiving it.

### Methods

#### 1. AddPeer

```rust
struct AddPeer {
    peer: Peer,
    sdp_offer: Option<String>,
    ice_servers: ICEServers
}
```

Related objects:
```rust
struct ICEServers {
    urls: Vec<String>,
    username: String,
    credential: String,
}
```

Servers requests [RTCPeerConnection] creation.

Params:
1. `peer`: peer connection settings.
2. `sdp_offer`: if `None`, client should create [SDP Offer] and pass it to the server. If `Some`, client should 
`setRemoteDescription`, create [SDP Answer] and pass it to the server.
3. `ice_servers`: just list of ice servers that should be passed to [RTCPeerConnection] constructor.

Peer settings should be discussed in more detail.

The most important part of `Peer` object is list of tracks. All `TrackDirection::Send` tracks must be created according 
to their settings and added to peer. If there is at least one `TrackDirection::Recv` track, then created 
[RTCPeerConnection] must be ready to receive tracks(`recvonly`/`sendrecv` SDP). Currently there are multiple ways to 
achieve this on client side and concrete implementation is not part of this RFC. 

#### Examples

1\. Create Audio+Video `sendrecv` p2p `Peer`.

```json
{
  "peer": {
    "peer_id": 1,
    "p2p": true,
    "tracks": [
      {
        "id": 1,
        "media_type": {
          "Audio": {}
        },
        "direction": {
          "Send": {
            "receivers": [ 2 ]
          }
        }
      },
      {
        "id": 2,
        "media_type": {
          "Video": {}
        },
        "direction": {
          "Send": {
            "receivers": [ 2 ]
          }
        }
      },
      {
        "id": 3,
        "media_type": {
          "Audio": {}
        },
        "direction": {
          "Recv": {
            "sender": 2
          }
        }
      },
      {
        "id": 4,
        "media_type": {
          "Video": {}
        },
        "direction": {
          "Recv": {
            "sender": 2
          }
        }
      }
    ]
  },
  "sdp_offer": null,
  "ice_servers": {
    "urls": [
      "turn:turnserver.com:3478",
      "turn:turnserver.com:3478?transport=tcp"
    ],
    "username": "turn_user",
    "credential": "turn_credential"
  }
}
```

Client is expected to:
1. Create [RTCPeerConnection] with provided ice servers and associate it with given `peer_id`.
2. Initialize Audio and Video tracks without any additional settings.
3. Add newly created tracks to [RTCPeerConnection].
4. Create `sendrecv` [SDP Offer].
5. Set offer as peers local description.
6. Answer `AddPeer` request with `Offer` request containing [SDP Offer].
7. Expect remote [SDP Answer] to set it as remote description.

After negotiation is done and media starts flowing, client will receive notification that his media is being sent to 
`Peer { peer_id = 2 }`, and he is receiving media from `Peer { peer_id = 2 }`.

2\. Create Audio `send` to SFU `Peer`.

```json
{
  "peer": {
    "peer_id": 1,
    "p2p": false,
    "tracks": [
      {
        "id": 1,
        "media_type": {
          "Audio": {}
        },
        "direction": {
          "Send": {
            "receivers": []
          }
        }
      }
    ]
  },
  "sdp_offer": "server_user1_recvonly_offer",
  "ice_servers": {
    "urls": [
      "turn:turnserver.com:3478",
      "turn:turnserver.com:3478?transport=tcp"
    ],
    "username": "turn_user",
    "credential": "turn_credential"
  }
}
```

Client is expected to:
1. Create [RTCPeerConnection] with provided ice servers and associate it with given `peer_id`.
2. Initialize Audio track without any additional settings.
3. Add newly created track to [RTCPeerConnection].
4. Set provided offer as peers remote description.
5. Create `sendonly` [SDP Answer].
6. Set created [SDP Answer] as local description.
7. Answer `AddPeer` request with `Answer` request containing [SDP Offer]. 

After negotiation is done and media starts flowing, client will receive notification that his media is being sent to 
server.


#### 2. RemovePeers

```rust
struct RemovePeers {
    peer_ids: Vec<u64>,
}
```

Server's/Client's request to dispose(close) specified `Peers`.

If Server => Client, then Client must dispose specified `Peers`.
If Client => Server, then Client requests Server's permission to dispose specified `Peers`. Server may give permission 
in answer.

Probably, Server will always give his permission on any Client's request. This kind of request flow will allow Server 
to do any request related stuff that Server needs to do, and distinguish between abnormal and normal events.

#### Examples

1. Server tells client to dispose specified `Peers` / Client requests Server's permission to dispose specified `Peers`.

```json
{
  "peer_ids": [ 1, 2, 3 ]
}
```

#### 3. UpdateTracks

```rust
struct UpdateTracks {
    peer_id: u64,
    tracks: Vec<Track>,
}
```

Server's/Client's request to update tracks in specified `Peer`.

If Server => Client, then it can be used to:
1. Add new track.
2. Update existing track settings (e.g. change to lower video resolution, mute audio).
3. Update send track receivers list (add/remove).

If Client => Server, then it can be used to express Clients intentions to:
1. Update existing track settings.
2. Cancel sending media to specific receiver (only remove).

#### Examples 

Assuming such `Peer` exists on Clients end:

```json
{
  "peer_id": 1,
  "p2p": false,
  "tracks": [
    {
      "id": 1,
      "media_type": {
        "Audio": {}
      },
      "direction": {
        "Send": {
          "receivers": []
        }
      }
    },
    {
      "id": 2,
      "media_type": {
        "Video": {}
      },
      "direction": {
        "Send": {
          "receivers": []
        }
      }
    }
  ]
}
```

Meaning that media is being published to server but has no actual receivers.

1\. Server notifies Client that video is being received by other `Peer {peer_id = 2}`.

Server => Client

```json
{
  "peer_id": 1,
  "tracks": [
    {
      "id": 1,
      "media_type": {
        "Audio": {}
      },
      "direction": {
        "Send": {
          "receivers": [ 2 ]
        }
      }
    },
    {
      "id": 2,
      "media_type": {
        "Video": {}
      },
      "direction": {
        "Send": {
          "receivers": [ 2 ]
        }
      }
    }
  ]
}
```

2\. Client wants to unsubscribe `Peer {peer_id = 2}` from specified tracks.

Client => Server

```json
{
  "peer_id": 1,
  "tracks": [
    {
      "id": 1,
      "media_type": {
        "Audio": {}
      },
      "direction": {
        "Send": {
          "receivers": []
        }
      }
    },
    {
      "id": 2,
      "media_type": {
        "Video": {}
      },
      "direction": {
        "Send": {
          "receivers": []
        }
      }
    }
  ]
}
```


#### 4. RemoveTracks

```rust
struct RemoveTracks {
    peer_id: u64,
    tracks: Vec<u64>,
}
```

Server's/Client's request to dispose specified `Tracks`.

If Server => Client, then Client must dispose(stop and remove).
If Client => Server, then Client requests Server's permission to dispose specified `Peers`.

#### Examples

1. Server tells client to dispose specified `Tracks` / Client requests Server's permission to dispose specified `Tracks`.

```json
{
  "peer_id": 1,
  "tracks": [1, 2]
}
```

#### 5. Offer

```rust
struct Offer {
    peer_id: u64,
    sdp_offer: String,
}
```

Server's / Client's [SDP Offer] sent during SDP negotiation between peers.

Client can send it:
1. As answer to `AddPeer {sdp_offer: None}`
2. As answer to `UpdateTracks` if update requires SDP renegotiation.

Server can send it:
1. If server triggers renegotiation.
2. Retransmission from peer that triggered renegotiation.

#### Examples

1. Client sends `Peers` [SDP Offer]

```json
{
  "peer_id": 1,
  "sdp_offer": "sdp_offer_body"
}
```

#### 6. Answer

```rust
struct Answer {
    peer_id: u64,
    sdp_answer: String,
}
```

Server's / Client's [SDP Answer]  sent during SDP negotiation between peers.

Client can send it:
1. As answer to `AddPeer {sdp_offer: Some}`.
2. As answer to `Offer`.

Server can send it only as answer to `Offer`.

#### Examples

1. Client sends `Peers` [SDP Answer]

```json
{
  "peer_id": 1,
  "sdp_offer": "sdp_answer_body"
}
```

#### 7. Candidate

```rust
struct Candidate {
    peer_id: u64,
    candidate: String,
}
```

Server's / Client's [ICE Candidate] sent during ICE negotiation.

Just send each [ICE Candidate] discovered by underlying [RTCPeerConnection] to remote `Peer`. It's as simple as that.

#### 8. RemotePeers

```rust
struct RemotePeers {
    peers: Vec<RemotePeer>
}
```

Related objects:

```rust
struct RemotePeer {
    remote_peer_id: Option<u64>,
    remote_member_id: Option<String>,
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

Server notifies Client of any remote peers that Client can connect to. This is a key method when talking about Dynamic 
API mentioned in `Signalling Protocol considerations`. Any Client's request to subscriber/publish will be based on data
provided by this request.

Params:
1. ```remote_peer_id```: if `Some`, then represents specific remote `Peer` associated with some `Member`. If `None`, then represents Servers peer connection.
2. ```remote_member_id```: if `Some`, then represents specific remote `Member` associated with some . If `None`, then represents Servers peer connection.
3. ```can_rx```: if `Some` then Client can subscribe to specified media.
4. ```can_tx```: if `Some` then Client can publish specified media to remote `Peer`.

#### Examples:

1. Notify Client that it is possible subscribe to `Member {id = 2}` Video and Audio tracks.

```json
{
  "peers": [
    {
      "peer_id": 2,
      "member_id": "User2",
      "can_rx": {
        "AudioVideo": {
          "audio_settings": {},
          "video_settings": {}
        }
      },
      "can_tx": null
    }
  ]
}
```

2. Notify Client that it is possible to publish Audio to specified `Peers`.

```json
{
  "peers": [
    {
      "peer_id": 2,
      "member_id": "User2",
      "can_rx": null,
      "can_tx": {
        "Audio": {
          "audio_settings": {}
        }
      }
    },
    {
      "peer_id": 3,
      "member_id": "User3",
      "can_rx": null,
      "can_tx": {
        "Audio": {
          "audio_settings": {}
        }
      }
    }
  ]
}
```

Params:
1. ```remote_peer_id```: if `Some`, then represents specific remote `Peer` associated with some `Member`. If `None`, 
then represents Servers peer connection (only SFU).
2. ```remote_member_id```: if `Some`, then represents specific remote `Member`. If `None`, then represents Server's peer 
connection (only SFU).
3. ```can_rx```: if `Some` then Client can subscribe to specified media.
4. ```can_tx```: if `Some` then Client can publish specified media to remote `Peer`.

#### 8. RequestTracks

```rust
struct RequestTracks {
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

Client requests to send or receive media to/from remote peer.

Params:
1. ```peer_id```: if `Some` then Client wants to connect specified local `Peer` to remote. If `None`then it us to server
to decide which of Client's `Peers` will be connected.
2. ```remote_peer_id```: if `Some`, then represents specific remote `Member` associated with some . If `None`, then 
represents Server's peer connection (only SFU).
3. ```rx```: if `Some` then Client requests to subscribe to specified media.
4. ```tx```: if `Some` then Client requests to publish specified media to remote `Peer`.

#### Examples

1\. Client requests to subscribe to remote `Peer {peer_id = 2}` audio and video.

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

2\. Client requests to publish to Server's peer connection.

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

#### 9. GetMembers, Members

```rust
struct GetMembers {
    peer_ids: Vec<u64>,
}
```

Client requests `Member` ids.

```rust
struct Members {
    members: Vec<Member>
}
```

Server provides `Member`'s list according to user request.

It is recommended to cache `Peer` id - `Member` id relation in Web Client. Probably, in two maps: `HashMap<peer_id, member_id>`, `HashMap<member_id, peer_id>`.

#### Examples

1\. Client request `Member`'s that own specified `Peer`'s.
```json
{
  "peer_ids": [
    2, 3, 4
  ]
}
```

2\. Server provides `Member`'s that own specified `Peers`'s.
```json
{
  "members": [
    {
      "member_id": "user_2",
      "peers": [ 1 ]
    },
    {
      "member_id": "user_2",
      "peers": [ 2 ]
    },
    {
      "member_id": "user_3",
      "peers": [ 3, 4 ]
    }
  ]
}
```

### Extended examples

#### 1. 1 <=> 1 p2p with unpublish and republish.   

```
.----user1----.    .->-->-->--. .----user2----.
:             o(1)=:          :=o(2)          :
'-------------'    '-<--<--<--' '-------------'
```

1\. Server send `AddPeer` to user1. 

```json
{
  "method": "AddPeer",
  "payload": {
    "peer": {
      "peer_id": 1,
      "p2p": true,
      "tracks": [
        {
          "id": 1,
          "media_type": {
            "Audio": {}
          },
          "direction": {
            "Send": {
              "receivers": [
                2
              ]
            }
          }
        },
        {
          "id": 2,
          "media_type": {
            "Video": {}
          },
          "direction": {
            "Send": {
              "receivers": [
                2
              ]
            }
          }
        },
        {
          "id": 3,
          "media_type": {
            "Audio": {}
          },
          "direction": {
            "Recv": {
              "sender": 2
            }
          }
        },
        {
          "id": 4,
          "media_type": {
            "Video": {}
          },
          "direction": {
            "Recv": {
              "sender": 2
            }
          }
        }
      ]
    },
    "sdp_offer": null,
    "ice_servers": {
      "urls": [
        "turn:turnserver.com:3478",
        "turn:turnserver.com:3478?transport=tcp"
      ],
      "username": "turn_user",
      "credential": "turn_credential"
    }
  }
}
```
 
2\. User1 answers with [SDP Offer]. 

```json
{
  "method": "Offer",
  "payload": {
    "peer_id": 1,
    "sdp_offer": "user1_sendrecv_offer"
  }
}
```
 
3\. Server send `AddPeer` with user1 [SDP Offer] to user2.

```json
{
  "method": "AddPeer",
  "payload": {
    "peer": {
      "peer_id": 2,
      "p2p": true,
      "tracks": [
        {
          "id": 1,
          "media_type": {
            "Audio": {}
          },
          "direction": {
            "Recv": {
              "sender": 1
            }
          }
        },
        {
          "id": 2,
          "media_type": {
            "Audio": {}
          },
          "direction": {
            "Recv": {
              "sender": 1
            }
          }
        },
        {
          "id": 3,
          "media_type": {
            "Audio": {}
          },
          "direction": {
            "Send": {
              "receivers": [ 1 ]
            }
          }
        },
        {
          "id": 4,
          "media_type": {
            "Video": {}
          },
          "direction": {
            "Send": {
              "receivers": [ 1 ]
            }
          }
        }
      ]
    },
    "sdp_offer": "user1_sendrecv_offer",
    "ice_servers": {
      "urls": [
        "turn:turnserver.com:3478",
        "turn:turnserver.com:3478?transport=tcp"
      ],
      "username": "turn_user",
      "credential": "turn_credential"
    }
  }
}
```

4\. User2 answers with [SDP Answer]

```json
{
  "method": "Answer",
  "payload": {
    "peer_id": 2,
    "sdp_answer": "user2_sendrecv_answer"
  }
}
```

5\. Both peers exchange discovered [ICE Candidate]'s.

```json
{
  "method": "Candidate",
  "payload": {
    "peer_id": 1,
    "candidate": "user1_ice_candidate"
  }
}
```

```json
{
  "method": "Candidate",
  "payload": {
    "peer_id": 2,
    "candidate": "user1_ice_candidate"
  }
}
```

6\. At this point connection is supposed to be established.

```
.----user1----.    .->-->-->--. .----user2----.
:             o(1)=:          :=o(2)          :
'-------------'    '-<--<--<--' '-------------'
```

7\. User1 wants to unpublish his tracks, so he sends `RemoveTracks` Server.

```json
{
  "method": "RemoveTracks",
  "payload": {
    "peer_id": 1,
    "tracks": [1, 2]
  }
}
```

8\. Server updates User2 tracks.

```json
{
  "method": "RemoveTracks",
  "payload": {
    "peer_id": 2,
    "tracks": [ 1, 2 ]
  }
}
```

9\. Server approves User1 `RemoveTracks` request.

10\. User1 initiates SDP renegotiation

```
.----user1----.         .----user2----.
:             o(1)-<--<-o(2)          :
'-------------'         '-------------'
```

11\. Server notifies User1 that he can publish to User2

```json
{
  "method": "RemotePeer",
  "payload": {
    "peers": [
      {
        "peer_id": 2,
        "member_id": "user_2",
        "can_rx": null,
        "can_tx": {
          "AudioVideo": {
            "audio_settings": {},
            "video_settings": {}
          }
        }
      }
    ]
  }
}
```

12\. Server notifies User2 that he can subscriber to User1.

```json
{
  "method": "RemotePeers",
  "payload": {
    "peers": [
      {
        "peer_id": 1,
        "member_id": "user_1",
        "can_rx": {
          "AudioVideo": {
            "audio_settings": {},
            "video_settings": {}
          }
        },
        "can_tx": null
      }
    ]
  }
}
```

13\. User1 requests to publish to User2.

```json
{
  "method": "RequestTracks",
  "payload": {
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

14\. Server updates User2 tracks.

```json
{
  "method": "UpdateTracks",
  "payload": {
    "peer_id": 2,
    "tracks": [
      {
        "id": 1,
        "media_type": {
          "Audio": {}
        },
        "direction": {
          "Recv": {
            "sender": 1
          }
        }
      },
      {
        "id": 2,
        "media_type": {
          "Audio": {}
        },
        "direction": {
          "Recv": {
            "sender": 1
          }
        }
      }
    ]
  }
}
```

15\. Server updates User1 tracks.

```json
{
  "method": "UpdateTracks",
  "payload": {
    "peer_id": 1,
    "tracks": [
      {
        "id": 1,
        "media_type": {
          "Audio": {}
        },
        "direction": {
          "Send": {
            "receivers": [ 2 ]
          }
        }
      },
      {
        "id": 2,
        "media_type": {
          "Video": {}
        },
        "direction": {
          "Send": {
            "receivers": [ 2 ]
          }
        }
      }
    ]
  }
}
```

16\. SDP Renegotiation

```
.----user1----.    .->-->-->--. .----user2----.
:             o(1)=:          :=o(2)          :
'-------------'    '-<--<--<--' '-------------'
```

#### 2. 1 => 2 SFU.

```
                                                       .-------user2------.
                          .-------SFU-------.    .-->--o      pc_id = 2   :
.------user1------.       :       .---->----o-->-'     '------------------'
:     pc_id = 1   o-->-->-o--->---:         :
'-----------------'       :       '---->----o-->-.     .-------user3------.
                          '-----------------'    '-->--o      pc_id = 3   :
                                                       '------------------'
```

1\. Server requests User1 to create `sendonly` `Peer` passing Server's [SDP Offer].

```json
{
  "method": "AddPeer",
  "payload": {
    "peer": {
      "peer_id": 1,
      "p2p": false,
      "tracks": [
        {
          "id": 1,
          "media_type": {
            "Audio": {}
          },
          "direction": {
            "Send": {
              "receivers": []
            }
          }
        },
        {
          "id": 2,
          "media_type": {
            "Video": {}
          },
          "direction": {
            "Send": {
              "receivers": []
            }
          }
        }
      ]
    },
    "sdp_offer": "server_user1_recvonly_offer",
    "ice_servers": {
      "urls": [
        "turn:turnserver.com:3478",
        "turn:turnserver.com:3478?transport=tcp"
      ],
      "username": "turn_user",
      "credential": "turn_credential"
    }
  }
}
```

2\. User1 creates peer and answeres with [SDP Answer].

```json
{
  "method": "Answer",
  "payload": {
    "peer_id": 1,
    "sdp_answer": "user_1_sendonly_answer"
  }
}
```

3\. Server and User1 exchange [ICE Candidate]'s.

```json
{
  "method": "Candidate",
  "payload": {
    "peer_id": 1,
    "candidate": "user1_ice_candidate"
  }
}
```

```json
{
  "method": "Candidate",
  "payload": {
    "peer_id": 1,
    "candidate": "servers_ice_candidate"
  }
}
```

4\. Connection is established

```
                          .-------SFU-------.
.------user1------.       :                 ;
:     pc_id = 1   o-->-->-o                 :
'-----------------'       :                 ;
                          '-----------------'
```

5\. Server requests User2 to create `recvonly` `Peer` passing Server's [SDP Offer]. 

```json
{
  "method": "AddPeer",
  "payload": {
    "peer": {
      "peer_id": 2,
      "p2p": false,
      "tracks": [
        {
          "id": 1,
          "media_type": {
            "Audio": {}
          },
          "direction": {
            "Recv": {
              "sender": 1
            }
          }
        },
        {
          "id": 2,
          "media_type": {
            "Video": {}
          },
          "direction": {
            "Recv": {
              "sender": 1
            }
          }
        }
      ]
    },
    "sdp_offer": "server_user2_sendonly_offer",
    "ice_servers": {
      "urls": [
        "turn:turnserver.com:3478",
        "turn:turnserver.com:3478?transport=tcp"
      ],
      "username": "turn_user",
      "credential": "turn_credential"
    }
  }
}
```

6\. User2 answers with [SDP Answer].

```json
{
  "method": "Answer",
  "payload": {
    "peer_id": 2,
    "sdp_answer": "user_2_recvonly_answer"
  }
}
```

7\. Server and User2 exchange [ICE Candidate]'s.

```json
{
  "method": "Candidate",
  "payload": {
    "peer_id": 2,
    "candidate": "user1_ice_candidate"
  }
}
```

```json
{
  "method": "Candidate",
  "payload": {
    "peer_id": 2,
    "candidate": "servers_ice_candidate"
  }
}
```

8\. User2 is connected to Server's peer connection.

```
                                                       .-------user2------.
                          .-------SFU-------.    .-->--o      pc_id = 2   :
.------user1------.       :                 o-->-'     '------------------'
:     pc_id = 1   o-->-->-o-                :
'-----------------'       :                 :
                          '-----------------'
```

9\. Server notifies User1 that he has new subscriber.

```json
{
  "method": "UpdateTracks",
  "payload": {
    "peer_id": 1,
    "tracks": [
      {
        "id": 1,
        "media_type": {
          "Audio": {}
        },
        "direction": {
          "Send": {
            "receivers": [ 2 ]
          }
        }
      },
      {
        "id": 2,
        "media_type": {
          "Video": {}
        },
        "direction": {
          "Send": {
            "receivers": [2]
          }
        }
      }
    ]
  }
}
```

10\. Server sends User1 `Peer {peer_id = 1 }` media to User2 `Peer {peer_id = 2 }`.

 ```
                                                        .-------user2------.
                           .-------SFU-------.    .-->--o      pc_id = 2   :
 .------user1------.       :         .->-->--o-->-'     '------------------'
 :     pc_id = 1   o-->-->-o--->-->--'       :
 '-----------------'       :                 :
                           '-----------------'
 ```
 
11\. Server requests User3 to create `recvonly` `Peer` passing Server's [SDP Offer]. 

```json
{
  "method": "AddPeer",
  "payload": {
    "peer": {
      "peer_id": 3,
      "p2p": false,
      "tracks": [
        {
          "id": 1,
          "media_type": {
            "Audio": {}
          },
          "direction": {
            "Recv": {
              "sender": 1
            }
          }
        },
        {
          "id": 2,
          "media_type": {
            "Video": {}
          },
          "direction": {
            "Recv": {
              "sender": 1
            }
          }
        }
      ]
    },
    "sdp_offer": "server_user3_sendonly_offer",
    "ice_servers": {
      "urls": [
        "turn:turnserver.com:3478",
        "turn:turnserver.com:3478?transport=tcp"
      ],
      "username": "turn_user",
      "credential": "turn_credential"
    }
  }
}
```

12\. User3 answers with [SDP Answer].

```json
{
  "method": "Answer",
  "payload": {
    "peer_id": 3,
    "sdp_answer": "user_3_recvonly_answer"
  }
}
```

13\. Server and User3 exchange [ICE Candidate]'s.

```json
{
  "method": "Candidate",
  "payload": {
    "peer_id": 3,
    "candidate": "user1_ice_candidate"
  }
}
```

```json
{
  "method": "Candidate",
  "payload": {
    "peer_id": 3,
    "candidate": "servers_ice_candidate"
  }
}
```

14\. User3 is connected to Server's peer connection.

```
                                                       .-------user2------.
                          .-------SFU-------.    .-->--o      pc_id = 2   :
.------user1------.       :       .---->----o-->-'     '------------------'
:     pc_id = 1   o-->-->-o--->---'         :
'-----------------'       :                 o-->-.     .-------user3------.
                          '-----------------'    '-->--o      pc_id = 3   :
                                                       '------------------'
```

15\. Server notifies User1 that he has new subscriber.

```json
{
  "method": "UpdateTracks",
  "payload": {
    "peer_id": 1,
    "tracks": [
      {
        "id": 1,
        "media_type": {
          "Audio": {}
        },
        "direction": {
          "Send": {
            "receivers": [ 2,3 ]
          }
        }
      },
      {
        "id": 2,
        "media_type": {
          "Video": {}
        },
        "direction": {
          "Send": {
            "receivers": [ 2,3 ]
          }
        }
      }
    ]
  }
}
```

16\. Server sends User1 `Peer {peer_id = 1 }` media to User2 `Peer {peer_id = 3 }`.

```
                                                     .-------user2------.
                          .-------SFU-------.    .->-o      pc_id = 2   :
.------user1------.       :       .---->----o-->-'   '------------------'
:     pc_id = 1   o-->-->-o--->---:         :
'-----------------'       :       '---->----o-->-.   .-------user3------.
                          '-----------------'    '->-o      pc_id = 3   :
                                                     '------------------'
```

## Drawbacks and alternatives
[drawbacks-and-alternatives]: #drawbacks-and-alternatives

This RFC design tries to be a "silver bullet": cover all possible use-cases and combine them in single protocol. Such 
versatility increases complexity. Simplifications can be achieved by imposing some general constraints:
1. Divide current protocl protocol in two separate protocols: one for SFU and one for P2P.
2. Reject future possibilities of using 1 `Peer` for all inbound/outbound tracks.
3. Limit number of outbound streams in single `Peer` to 1.
4. Remove publishers acknowledgement of every receiver on each track.
5. Remove subscribers acknowledgement of every publisher that is not publishing at the moment.

## Unresolved questions and future possibilities
[unresolved-questions-and-future-possibilities]: #unresolved-questions-and-future-possibilities

### Data channels

[WebRTC] spec introduces [RTCDataChannel] - a bi-directional data channel between two peers which allows arbitrary data 
exchange. It is an amazing feature with huge potential, but, at this point it is quite useless for our use cases.

As the project develops, requirements will change, and we might consider adding data channels. Although, they are not 
mentioned in this protocol, only minor tweaks will be required to support them.

### Receiving tracks form multiple senders in single peer connection

There are two general ways to manage Client's peer connections when using SFU server:
1. Having only one pair of [RTCPeerConnection]'s (one at Client end and one at Server) and passing all send/recv tracks 
through this connection.
2. Having a separate [RTCPeerConnection] pair for each track group.

First way is preferable since it allows to reduce resources usage on both ends. But track management is very unclear in 
this case and gstreamers [webrtcbin] currently does not support dynamic addition/removal of streams and needs major 
updates to be able to do so.

Current protocol assumes that there will be separate [RTCPeerConnection] pair for each track group. 
At the same time, it does not forbid having all the tracks in single [RTCPeerConnection] pair, but it will require some 
minor changes to make this work.




[RTCPeerConnection]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
[MediaStreamTrack]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
[webrtcbin]: https://gstreamer.freedesktop.org/data/doc/gstreamer/head/gst-plugins-bad/html/gst-plugins-bad-plugins-webrtcbin.html
[RTCDataChannel]:https://www.w3.org/TR/webrtc/#rtcdatachannel
[WebRTC]:https://www.w3.org/TR/webrtc/
[SDP Offer]:https://tools.ietf.org/html/rfc3264
[SDP Answer]:https://tools.ietf.org/html/rfc3264
[ICE Candidate]:https://tools.ietf.org/html/rfc8445