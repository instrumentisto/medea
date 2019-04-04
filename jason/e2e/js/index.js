async function f() {
    const rust = await import("../../pkg");

    let jason = new rust.Jason();

    let session = jason.init_session("ws://localhost:8080/ws/1/1/caller_credentials");

    alert("hi");
}

f();