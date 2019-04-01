import("../../pkg").then(rust => {
  let medea = new rust.Medea("ws://localhost:8080/ws/1/1/caller_credentials");
  console.log(medea.get_token());
});
