async function f() {
    console.log(document);
    const rust = await import("../../pkg");

    let caller = new rust.Jason();

    let caller_room_handle = await caller.join_room("ws://localhost:8080/ws/1/1/caller_credentials");

    caller_room_handle.on_local_stream(function (stream, error) {
        if (stream) {
            console.log(stream);
            var video = document.createElement("video");


            console.log(stream.get_audio_track());
            console.log(stream.get_video_track());

            video.srcObject = stream.get_audio_track();
            video.srcObject = stream.get_video_track();

            console.log(video);

            document.body.appendChild(video);
            console.log(document);
        } else {
            console.log(error);
        }
    });

    let responder = new rust.Jason();

    let responder_room_handler = await responder.join_room("ws://localhost:8080/ws/1/2/responder_credentials");

    // caller_room_handle.on_new_connection(function (connection) {
    //    connection.on_remote_stream(function (stream) {
    //
    //    });
    // });
    //
    // caller.dispose();
    // responder.dispose();
}

f();
