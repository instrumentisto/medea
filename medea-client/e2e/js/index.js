import("../../pkg").then(rust => {
  let medea = new rust.Medea("ws://localhost");
  console.log(medea.get_token());
});
