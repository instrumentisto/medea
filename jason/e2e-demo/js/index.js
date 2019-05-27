async function f() {
    const rust = await import("../../pkg");

    let caller = new rust.Jason();

    // let caller_room_handle = await caller.join_room("ws://localhost:8080/ws/1/1/caller_credentials");
    caller.join_room("ws://localhost:8080/ws/video-call-1/caller/test");
    caller.join_room("ws://localhost:8080/ws/video-call-1/responder/test");

    // Use this for testing with 3 members.
    // caller.join_room("ws://localhost:8080/ws/video-call-3/caller/test");
    // caller.join_room("ws://localhost:8080/ws/video-call-3/responder/test");
    // caller.join_room("ws://localhost:8080/ws/video-call-3/responder2/test");


    // caller.join_room("ws://localhost:8080/ws/1/2/responder_credentials");
    // let responder = new rust.Jason();

    // let responder_room_handler = await responder.join_room("ws://localhost:8080/ws/1/2/responder_credentials");

    // caller.dispose();
    // responder.dispose();
}

f();
