async function f() {
    console.log(document);
    const rust = await import("../../pkg");

    let caller = new rust.Jason();

    let caller_room_handle = await caller.join_room("ws://localhost:8080/ws/1/1/caller_credentials");

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

    caller_room_handle.on_new_connection(function (connection) {
        console.log("caller got new connection")
    });

    let responder = new rust.Jason();

    let responder_room_handler = await responder.join_room("ws://localhost:8080/ws/1/2/responder_credentials");

    responder_room_handler.on_new_connection(function (connection) {

        connection.on_remote_stream(function (stream) {
            console.log("got remote video");

            var video = document.createElement("video");

            video.srcObject = stream.get_media_stream();
            document.body.appendChild(video);
            video.play();
        });
    });

    // caller.dispose();
    // responder.dispose();
}

window.onload = async function () {
    await f();
};


