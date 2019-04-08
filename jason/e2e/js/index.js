async function f() {
    const rust = await import("../../pkg");

    let caller = new rust.Jason();

    let caller_session = caller.init_session("ws://localhost:8080/ws/1/1/caller_credentials");

    let responder = new rust.Jason();

    let responder_session = responder.init_session("ws://localhost:8080/ws/1/2/responder_credentials");

}

f();