let assert = chai.assert;

describe('Pub<=>Pub video call', () => {
    /**
     * Send POST request for create pub-pub-e2e-call to control-api-mock.
     */
    async function createRoom() {
        await axios({
            method: 'post',
            url: 'http://127.0.0.1:8000/pub-pub-e2e-call',
            data: {
            pipeline: {
                caller: {
                    kind: 'Member',
                    credentials: 'test',
                    pipeline: {
                        publish: {
                            kind: 'WebRtcPublishEndpoint',
                            spec: {
                                p2p: 'Always'
                            }
                        },
                        play: {
                            kind: 'WebRtcPlayEndpoint',
                            spec: {
                                src: 'local://pub-pub-e2e-call/responder/publish',
                            }
                        }
                    }
                },
                responder: {
                    kind: 'Member',
                    credentials: 'test',
                    pipeline: {
                        publish: {
                            kind: 'WebRtcPublishEndpoint',
                            spec: {
                                p2p: 'Always',
                            }
                        },
                        play: {
                            kind: 'WebRtcPlayEndpoint',
                            spec: {
                                src: 'local://pub-pub-e2e-call/caller/publish',
                            }
                        }
                    }
                }
            }
        }})
    }

    /**
     * Send DELETE pub-pub-e2e-call request to control-api-room.
     */
    async function deleteRoom() {
        await axios.delete('http://127.0.0.1:8000/pub-pub-e2e-call')
    }

    const callerPartnerVideo = 'callers-partner-video';
    const responderPartnerVideo = 'responder-partner-video';

    /**
     * Start Pub<=>Pub video call.
     *
     * This function returns caller room and responder room objects.
     */
    async function startPubPubVideoCall() {
        let caller = await window.getJason();
        let responder = await window.getJason();

        let callerRoom = await caller.join_room("ws://127.0.0.1:8080/ws/pub-pub-e2e-call/caller/test");
        let responderRoom = await responder.join_room("ws://127.0.0.1:8080/ws/pub-pub-e2e-call/responder/test");

        callerRoom.on_new_connection((connection) => {
            connection.on_remote_stream((stream) => {
                let video = document.createElement("video");
                video.id = callerPartnerVideo;

                video.srcObject = stream.get_media_stream();
                document.body.appendChild(video);
                video.play();
            });
        });
        caller.on_local_stream((stream, error) => {
            if (stream) {
                let video = document.createElement("video");

                video.srcObject = stream.get_media_stream();
                document.body.appendChild(video);
                video.play();
            } else {
                console.log(error);
            }
        });

        responder.on_local_stream((stream, error) => {
            if (stream) {
                let video = document.createElement("video");

                video.srcObject = stream.get_media_stream();
                document.body.appendChild(video);
                video.play();
            } else {
                console.log(error);
            }
        });
        responderRoom.on_new_connection((connection) => {
            connection.on_remote_stream(function(stream) {
                let video = document.createElement("video");
                video.id = responderPartnerVideo;

                video.srcObject = stream.get_media_stream();
                document.body.appendChild(video);
                video.play();
            });
        });

        return {
            caller: callerRoom,
            responder: responderRoom
        }
    }

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

    it('send rtc packets', async () => {
        /**
         * Takes array of RTCStatsReport and count "outbound-rtp" and "inbound-rtp"
         * for all RTCStatsReport. If "outbound-rtp"'s "packetsSent" or "inbound-rtp"'s
         * "packetsReceived" < 5 then test failed.
         * @param stats array of RTCStatsReports
         */
        function checkStats(stats) {
            let outboundPackets = 0;
            let inboundPackets = 0;
            stats.forEach(resp => {
                resp.forEach(report => {
                    if (report.type === 'outbound-rtp') {
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
    }).retries(5);

    it('video not static', async () => {
        let callerVideo = await waitForElement(callerPartnerVideo);
        checkVideoDiff(callerVideo);
        let responderVideo = await waitForElement(responderPartnerVideo);
        checkVideoDiff(responderVideo)
    }).retries(5);

    it('media tracks count valid', async () => {
        let callerVideo = await waitForElement(callerPartnerVideo);
        assert.lengthOf(callerVideo.srcObject.getTracks(), 2, "Caller video don't have 2 tracks");
        let responderVideo = await waitForElement(responderPartnerVideo);
        assert.lengthOf(responderVideo.srcObject.getTracks(), 2, "Responder video don't have 2 tracks");
    })
});
