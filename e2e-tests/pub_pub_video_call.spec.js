describe('Pub<=>Pub video call', () => {
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
        await send_rtc_packets_test(rooms)
    }).retries(5);

    it('video not static', async () => {
        await video_not_static_test()
    }).retries(5);

    it('media tracks count valid', async () => {
        await media_track_count_valid_test()
    })
});
