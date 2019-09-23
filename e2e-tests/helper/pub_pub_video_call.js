let assert = chai.assert;

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

async function video_not_static_test() {
    let callerVideo = await waitForElement(callerPartnerVideo);
    checkVideoDiff(callerVideo);
    let responderVideo = await waitForElement(responderPartnerVideo);
    checkVideoDiff(responderVideo)
}

async function media_track_count_valid_test() {
    let callerVideo = await waitForElement(callerPartnerVideo);
    assert.lengthOf(callerVideo.srcObject.getTracks(), 2, "Caller video don't have 2 tracks");
    let responderVideo = await waitForElement(responderPartnerVideo);
    assert.lengthOf(responderVideo.srcObject.getTracks(), 2, "Responder video don't have 2 tracks");
}

async function send_rtc_packets_test(rooms) {
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
}