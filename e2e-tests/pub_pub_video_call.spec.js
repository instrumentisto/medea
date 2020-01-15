let assert = chai.assert;

describe('Pub<=>Pub video call', function() {

  this.timeout(20000);

  /**
   * Sends POST request for create 'pub-pub-e2e-call' with 'control-api-mock'.
   */
  async function createRoom() {
    await axios({
      method: 'post',
      url: 'http://127.0.0.1:8000/control-api/pub-pub-e2e-call',
      data: {
        kind: 'Room',
        pipeline: {
          caller: {
            kind: 'Member',
            credentials: 'test',
            pipeline: {
              publish: {
                kind: 'WebRtcPublishEndpoint',
                p2p: 'Always'
              },
              play: {
                kind: 'WebRtcPlayEndpoint',
                src: 'local://pub-pub-e2e-call/responder/publish',
              }
            }
          },
          responder: {
            kind: 'Member',
            credentials: 'test',
            pipeline: {
              publish: {
                kind: 'WebRtcPublishEndpoint',
                p2p: 'Always',
              },
              play: {
                kind: 'WebRtcPlayEndpoint',
                src: 'local://pub-pub-e2e-call/caller/publish',
              }
            }
          }
        }
      }
    })
  }

  /**
   * Send DELETE 'pub-pub-e2e-call' request to a 'medea-control-api-mock' server.
   */
  async function deleteRoom() {
    await axios.delete(
      'http://127.0.0.1:8000/control-api/pub-pub-e2e-call'
    )
  }

  const callerPartnerVideo = 'callers-partner-video';
  const responderPartnerVideo = 'responder-partner-video';

  /**
   * Creates new 'Room' which will add video of partner to a 'document.body'
   * with provided ID.
   */
  async function newRoom(id) {
    let jason = await window.getJason();
    room = await jason.init_room();

    room.on_new_connection((connection) => {
      connection.on_remote_stream(async (stream) => {
        let video = document.createElement("video");
        video.id = id;

        video.srcObject = stream.get_media_stream();
        document.body.appendChild(video);
        await video.play();
      });
    });

    room.on_failed_local_stream((error) => {
      throw Error("Failed local stream. " + error
        .message());
    });

    room.on_close(function(on_closed) {
      throw Error("on_closed");
    });

    return room;
  }

  /**
   * Starts 'Pub<=>Pub' video call.
   G
   * Returns 'Room' for 'caller' and 'responder'.
   */
  async function startPubPubVideoCall() {
    const callerRoom = await newRoom(callerPartnerVideo);
    const responderRoom = await newRoom(responderPartnerVideo);

    await callerRoom.join(
      "ws://127.0.0.1:8080/ws/pub-pub-e2e-call/caller/test"
    );
    await responderRoom.join(
      "ws://127.0.0.1:8080/ws/pub-pub-e2e-call/responder/test"
    );

    return {
      caller: callerRoom,
      responder: responderRoom
    }
  }

  // Rooms with which tests will ran.
  let rooms;

  before(async () => {
    await deleteRoom();
    await createRoom();
    rooms = await startPubPubVideoCall();
    let video = await waitForElement(callerPartnerVideo);
    await waitForVideo(video);
  });

  after(async () => {
    await deleteRoom();
  });

  it('sends rtc packets', async () => {
    /**
     * Takes array of RTCStatsReport and count 'outbound-rtp' and 'inbound-rtp"
     * for all RTCStatsReport. If 'outbound-rtp''s 'packetsSent' or 'inbound-rtp"'s
     * "packetsReceived" < 5 then test failed.
     * @param stats array of RTCStatsReports
     */
    function checkStats(stats) {
      let outboundPackets = 0;
      let inboundPackets = 0;
      stats.forEach(resp => {
        resp.forEach(report => {
          if (report.type ===
            'outbound-rtp'
          ) {
            outboundPackets += report.packetsSent;
          } else if (report.type === 'inbound-rtp') {
            inboundPackets += report.packetsReceived;
          }
        });
      });
      assert.isAtLeast(outboundPackets, 5, 'outbound-rtp packets not sending');
      assert.isAtLeast(inboundPackets, 5, 'inbound-rtp packets not sending');
    }

    let callerStats = await rooms.caller.get_stats_for_peer_connections();
    checkStats(callerStats);
    let responderStats = await rooms.responder.get_stats_for_peer_connections();
    checkStats(responderStats);
  }).retries(20);

  it('videos not static', async () => {
    let callerVideo = await waitForElement(callerPartnerVideo);
    checkVideoDiff(callerVideo);
    let responderVideo = await waitForElement(responderPartnerVideo);
    checkVideoDiff(responderVideo);
  }).retries(20);

  it('media tracks count valid', async () => {
    let callerVideo = await waitForElement(callerPartnerVideo);
    assert.lengthOf(
        callerVideo.srcObject.getTracks(),
        2,
        "Caller video don't have 2 tracks"
    );
    let responderVideo = await waitForElement(responderPartnerVideo);
    assert.lengthOf(
        responderVideo.srcObject.getTracks(),
        2,
        "Responder video don't have 2 tracks"
    );
  }).retries(20)
});
