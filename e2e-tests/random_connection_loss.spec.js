describe('Pub<=>Pub video call', () => {
    let rooms;

    before(async function() {
        this.timeout(60000);
        await deleteRoom();
        await createRoom();
        rooms = await startPubPubVideoCall();
        let video = await waitForElement(callerPartnerVideo);
        await waitForVideo(video);
    });

    after(async () => {
        await deleteRoom();
        await axios.post("http://127.0.0.1:8500/gremlin/stop");
    });

    /**
     * Start Pub<=>Pub video call.
     *
     * This function returns caller room and responder room objects.
     */
    async function startPubPubVideoCall() {
        let caller = await window.getJason();
        let responder = await window.getJason();

        let callerRoom = await caller.join_room("ws://127.0.0.1:8080/ws/pub-pub-e2e-call/caller/test");
        let responderRoom = await responder.join_room("ws://127.0.0.1:8090/ws/pub-pub-e2e-call/responder/test");

        responderRoom.on_event(async (e) => {
            if (e.event === 'PeerCreated') {
                await axios.post('http://127.0.0.1:8500/gremlin/start');
            }
        });

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

    it('send rtc packets', async () => {
        await send_rtc_packets_test(rooms)
    }).retries(10000);

    it('video not static', async () => {
        await video_not_static_test()
    }).retries(10000);

    it('media tracks count valid', async () => {
        await media_track_count_valid_test()
    }).retries(10000)
});
