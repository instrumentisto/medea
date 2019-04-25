async function f() {
    const rust = await import("../../pkg");

    let caller = new rust.Jason();

    caller.join_room("ws://localhost:8080/ws/1/1/caller_credentials");

    let responder = new rust.Jason();

    responder.join_room("ws://localhost:8080/ws/1/2/responder_credentials");
}

f();