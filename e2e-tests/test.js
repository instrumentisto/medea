let assert = chai.assert;

function delay(interval)
{
    return it('should delay', done =>
    {
        setTimeout(() => done(), interval)

    }).timeout(interval + 100)
}

describe('Some dummy test', () => {
    before(async () => {
        let caller = await window.getJason();
        let responder = await window.getJason();

        let callerRoom = await caller.join_room("ws://localhost:8080/ws/pub-pub-e2e-call/caller/test");
        let responderRoom = await responder.join_room("ws://localhost:8080/ws/pub-pub-e2e-call/responder/test");

        callerRoom.on_new_connection((connection) => {
            console.log("caller got new connection with member " + connection.member_id());
            connection.on_remote_stream((stream) => {
                console.log("got video from remote member " + connection.member_id());

                let video = document.createElement("video");
                video.id = 'callers-partner-video';

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
            console.log("responder got new connection with member " + connection.member_id());
            connection.on_remote_stream(function(stream) {
                console.log("got video from remote member " + connection.member_id());

                let video = document.createElement("video");
                video.id = 'responders-partner-video';

                video.srcObject = stream.get_media_stream();
                document.body.appendChild(video);
                video.play();
            });
        });
    })

    after(() => {
        let successEl = document.createElement('div');
        successEl.id = 'test-end';
        document.body.appendChild(successEl);
    });

    delay(2000);

    it('success', () => {
        assert.equal('bar', 'bar');
    })
});
