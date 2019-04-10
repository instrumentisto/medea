async function f() {
  const rust = await import("../../pkg");

  let caller = new rust.Jason();

  caller.init_session("ws://localhost:8080/ws/1/1/caller_credentials");

  let responder = new rust.Jason();

  responder.init_session("ws://localhost:8080/ws/1/2/responder_credentials");
}

f();