async function f() {
    const rust = await import("../../pkg");

    let caller = new rust.Jason();

    // let caller_room_handle = await caller.join_room("ws://localhost:8080/ws/1/1/caller_credentials");
    caller.join_room("ws://localhost:8080/ws/1/1/caller_credentials");

    // let responder = new rust.Jason();

    // let responder_room_handler = await responder.join_room("ws://localhost:8080/ws/1/2/responder_credentials");

    caller.dispose();
    // responder.dispose();
}

f();
