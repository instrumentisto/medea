- Feature Name: `client_webrtc_signalling_api`
- Start Date: 2018-12-13
- RFC PR: (leave this empty)
- Tracking Issue: (leave this empty)




## Summary
[summary]: #summary

Formalize communication protocol between client(browser, mobile apps) and media server regarding WebRTC connection management.

## Motivation
[motivation]: #motivation

WebRTC allows P2P data exchange, but WebRTC as a protocol comes without signaling. At a minimum signalling protocol must provide ways to exchange Session Description 
data(SDP offer/answer) and ICE Candidates. But if you think about signalling protocol in terms of interaction with media server things are becoming more complicated.
You will need to express ways to:
1. Provide STUN/TURN servers.
2. Exchange some low-level media metadata(resolution, codecs, media types).
3. Allow more sophisticated track management(updating video resolution on preview/fullscreen switches, passing multiple video tracks with different settings).
4. Pass some user metadata to hook business logic on.
5. Build more complex connection graphs.
6. Dynamically cancel/begin media publishing/receiving.
7. Passing erros, connection stats messages.
8. Cover both P2P mesh and SFU scenarios.

The protocol must be versatile enough to cover all possible use-cases.

## Guide-level explanation
[guide-level-explanation]: #guide-level-explanation

### What is `WebRTC Client API`? 

It is a part of `Client API` responsible for WebRTC connection management. You can find `Client API` on approximate architecture design. 

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
Although, signalling can be implemented on top of any transport, WebSocket is suited the most since it provides small overhead reliable duplex connection. Widely used and supported.

### Protocol considerations

Existing best practices are recommended:
1. Message level ping-pongs.
2. Reconnects.
3. Transactions.
4. Correct usage of close frames.

### Signalling Protocol considerations

One of the main goals, is to make `Medea Web Client` integration as easy as possible. This means less interaction between `User Application` and `Medea Web Client` 
and more interaction between `Medea Web Client` and `Medea Media Server`, quite verbose `Control Api` design.

Having in mind, that `Medea Media Server` already has user connection graph received from `Control Service` by the moment user connects, 
it is possible to establish all required connections without bothering `User Application`. Basically connection establishment should not depend on interaction with `User Application`.

On the other hand, some use cases require more manual control over medea exchange process. For example:
1. User wants to receive lower resolution video.
2. User wants to stop sending media to specific user.
3. Mute/unmute.

So API can be divided in two categories:
1. Preconfigured: where everything works from the box and almost no interaction between `User Application` and `Medea Web Client` required.
2. Dynamic: when `User Application` needs to express complex use cases.

## Reference-level explanation
[reference-level-explanation]: #reference-level-explanation

### Used primitives:

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

Just a way to group `Peers` and provide `User Application` with some users meta data.

```
struct Member {
    member_id: String,
    peers: Vec<u64>,
}
```

#### Peer

[RTCPeerConnection] representation.

```
struct Peer {
    peer_id: u64,
    p2p: bool,
    tracks: Vec<Track>,
}
```

#### Track

Somewhat [MediaStreamTrack] representation.

```
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

`P2P` flag implies some logic on `TrackDirection::Send`:
1. `P2P` send tracks always have only one receiver.
2. Non `P2P` send tracks can have 0-N receivers. 0 - if media is transmitted to server, but have no actual user receiving it.

### Methods


#### AddPeer
```
struct AddPeer {
    peer: Peer,
    sdp_offer: Option<String>,
    ice_servers: Vec<String>
}
```

Servers requests [RTCPeerConnection] creation.

Params:
1. `peer` : peer connection settings. The most important part of Peer struct is list of tracks. All `TrackDirection::Send` tracks must be created according to their settings and added to peer. If there is at least one `TrackDirection::Recv` track, then 
2. `sdp_offer` : if `None`, client should create offer and pass it to the server. If `Some`, client should `setRemoteDescription`, create answer and pass it to the server.
3. `ice_servers` : just list of ice servers that should be passed to [RTCPeerConnection] constructor.



## Drawbacks
[drawbacks]: #drawbacks

Why should we *not* do this?




## Rationale and alternatives
[rationale-and-alternatives]: #rationale-and-alternatives

- Why is this design the best in the space of possible designs?
- What other designs have been considered and what is the rationale for not choosing them?
- What is the impact of not doing this?



## Unresolved questions
[unresolved-questions]: #unresolved-questions

Some low-level details 




## Future possibilities
[future-possibilities]: #future-possibilities

Think about what the natural extension and evolution of your proposal would be and how it would affect the project as a whole in a holistic way. Try to use this section as a tool to more fully consider all possible interactions with the project in your proposal. Also consider how the this all fits into the roadmap for the project and of the relevant sub-team.

This is also a good place to "dump ideas", if they are out of scope for the RFC you are writing but otherwise related.

If you have tried and cannot think of any future possibilities, you may simply state that you cannot think of anything.

Note that having something written down in the future-possibilities section is not a reason to accept the current or a future RFC; such notes should be in the section on [motivation] or [rationale][rationale-and-alternatives] in this or subsequent RFCs. The section merely provides additional information.


[RTCPeerConnection]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
[MediaStreamTrack]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack