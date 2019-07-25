let assert = chai.assert;
let expect = chai.expect;

function delay(interval)
{
    return it('should delay', done =>
    {
        setTimeout(() => done(), interval)

    }).timeout(interval + 100)
}

describe('Pub<=>Pub video call', () => {
    const sleep = (milliseconds) => {
        return new Promise(resolve => setTimeout(resolve, milliseconds))
    };

    const waitForElement = (id) => {
        return new Promise(resolve => {
            let interval = setInterval(() => {
                let waitedEl = document.getElementById(id);
                if(waitedEl != null) {
                    clearInterval(interval);
                    resolve(waitedEl);
                }
            }, 50)
        })
    };

    const waitForVideo = (videoEl) => {
        return new Promise(resolve => {
            let interval = setInterval(() => {
                if(videoEl.videoWidth !== 0) {
                    clearInterval(interval);
                    resolve()
                }
            }, 50)
        })
    };

    const callerPartnerVideo = 'callers-partner-video';
    const responderPartnerVideo = 'responder-partner-video';

    async function startPubPubVideoCall() {
        let caller = await window.getJason();
        let responder = await window.getJason();

        let callerRoom = await caller.join_room("ws://localhost:8080/ws/pub-pub-e2e-call/caller/test");
        let responderRoom = await responder.join_room("ws://localhost:8080/ws/pub-pub-e2e-call/responder/test");

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
        rooms = await startPubPubVideoCall();
        let video = await waitForElement(callerPartnerVideo);
        await waitForVideo(video);
    });

    after(() => {
        let successEl = document.createElement('div');
        successEl.id = 'test-end';
        document.body.appendChild(successEl);
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
    });

    it('video not static', async () => {
        /**
         * Return difference between two arrays.
         *
         * In this test it's used for comparing images received from partner.
         *
         * @param o first array
         * @param n second array
         * @returns {number} number of how arrays are different
         */
        function diff(o, n) {
            let objO = {},
                objN = {};
            for (let i = 0; i < o.length; i++) {
                objO[o[i]] = 1;
            }
            for (let i = 0; i < n.length; i++) {
                objN[n[i]] = 1;
            }
            let added = 0;
            let removed = 0;

            for (let i in objO) {
                if (i in objN) {
                    delete objN[i];
                } else {
                    removed += 1;
                }
            }
            for (let i in objN) {
                added += 1;
            }

            return added + removed
        }

        /**
         * Get two images from provided video element with some small interval
         * and check that they are different.
         *
         * Test will fail if difference between this two images are less than 50.
         *
         * Use for testing that video which we receiving from partner are not static.
         *
         * @param videoEl video element
         */
        async function checkVideoDiff(videoEl) {
            let canvas = document.createElement('canvas');
            canvas.height = videoEl.videoHeight / 2;
            canvas.width = videoEl.videoWidth / 2;

            let context = canvas.getContext('2d');
            context.drawImage(videoEl, canvas.width, canvas.height, canvas.width, canvas.height);
            let imgEl = document.createElement('img');
            imgEl.src = canvas.toDataURL();
            let firstData = context.getImageData(0, 0, canvas.width, canvas.height);

            context.drawImage(videoEl, 0, 0, canvas.width, canvas.height);
            imgEl.src = canvas.toDataURL();
            let secondData = context.getImageData(0, 0, canvas.width, canvas.height);

            let dataDiff = diff(firstData.data, secondData.data);

            assert.isAtLeast(dataDiff, 10, 'Video which we receiving from partner looks static.');
        }

        let callerVideo = await waitForElement(callerPartnerVideo);
        await checkVideoDiff(callerVideo);
        let responderVideo = await waitForElement(responderPartnerVideo);
        await checkVideoDiff(responderVideo)
    });

    it('media tracks count valid', async () => {
        let callerVideo = await waitForElement(callerPartnerVideo);
        assert.lengthOf(callerVideo.srcObject.getTracks(), 2, "Caller video don't have 2 tracks");
        let responderVideo = await waitForElement(responderPartnerVideo);
        assert.lengthOf(responderVideo.srcObject.getTracks(), 2, "Responder video don't have 2 tracks");
    })
});
