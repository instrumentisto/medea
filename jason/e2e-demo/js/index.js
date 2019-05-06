async function f() {
    const rust = await import("../../pkg");

    let caller = new rust.Jason();

    let caller_room_handle = await caller.join_room("ws://localhost:8080/ws/1/1/caller_credentials");



    caller_room_handle.on_local_stream(function (stream, error) {
        console.log(error);
    });

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
