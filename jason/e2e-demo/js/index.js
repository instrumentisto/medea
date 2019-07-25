var caller_room;
var responder_room;

async function f() {
    const rust = await import("../../pkg");

    let caller = new rust.Jason();
    let responder = new rust.Jason();

    caller_room = await caller.join_room("ws://localhost:8080/ws/1/1/caller_credentials");
    responder_room = await responder.join_room("ws://localhost:8080/ws/1/2/responder_credentials");

    caller_room.on_new_connection(function (connection) {
        console.log("caller got new connection with member " + connection.member_id());
        connection.on_remote_stream(function (stream) {
            console.log("got video from remote member " + connection.member_id());

            var video = document.createElement("video");

            video.srcObject = stream.get_media_stream();
            document.body.appendChild(video);
            video.play();
        });
    });
    caller.on_local_stream(function (stream, error) {
        if (stream) {
            var video = document.createElement("video");

            video.srcObject = stream.get_media_stream();
            document.body.appendChild(video);
            video.play();
        } else {
            console.log(error);
        }
    });

    responder.on_local_stream(function (stream, error) {
        if (stream) {
            var video = document.createElement("video");

            video.srcObject = stream.get_media_stream();
            document.body.appendChild(video);
            video.play();
        } else {
            console.log(error);
        }
    });
    responder_room.on_new_connection(function (connection) {
        console.log("responder got new connection with member " + connection.member_id());
        connection.on_remote_stream(function (stream) {
            console.log("got video from remote member " + connection.member_id());

            var video = document.createElement("video");

            video.srcObject = stream.get_media_stream();
            document.body.appendChild(video);
            video.play();
        });
    });
}

window.onload = async function () {
    await f();
    setTimeout(function() {
        caller_room.mute_audio();
        caller_room.mute_video();
        responder_room.mute_audio();
        responder_room.mute_video();

        setTimeout(function() {
            caller_room.unmute_audio();
            caller_room.unmute_video();
        }, 2000);

        setTimeout(function() {
            responder_room.unmute_audio();
            responder_room.unmute_video();
        }, 3000);
    }, 5000);
};


