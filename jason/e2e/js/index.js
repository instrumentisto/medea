import("../../pkg").then(rust => {
  let jason = new rust.Jason("ws://localhost:8080/ws/1/1/caller_credentials");
  console.log(jason.get_token());
});
