async function f() {
    const rust = await import("../../pkg");

    let caller = new rust.Jason();
    let responder = new rust.Jason();

    let caller_room_handle = await caller.join_room("ws://localhost:8080/ws/1/1/caller_credentials");
    let responder_room_handler = await responder.join_room("ws://localhost:8080/ws/1/2/responder_credentials");

    caller_room_handle.on_new_connection(function (connection) {
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
    responder_room_handler.on_new_connection(function (connection) {
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
};


