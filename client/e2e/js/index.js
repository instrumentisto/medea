import("../../medea-client/pkg").then(rust => {
  let medea = new rust.Medea("some_token");
  console.log(medea.get_token());
});
