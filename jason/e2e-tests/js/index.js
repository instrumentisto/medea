async function f() {
  const rust = await import("../../pkg");

  let caller = new rust.Jason();
  let responder = new rust.Jason();

  let caller_room = await caller.join_room("ws://localhost:8080/ws/pub-pub-video-call/caller/test");
  let responder_room = await responder.join_room("ws://localhost:8080/ws/pub-pub-video-call/responder/test");

  caller_room.on_new_connection(function (connection) {
    window.medeaEvents.push("test");
    window.medeaEvents.push("sakljdfkljahsdkjfdsaf");
    console.log("caller got new connection with member " + connection.member_id());
    connection.on_remote_stream(function (stream) {
      console.log("got video from remote member " + connection.member_id());

      var video = document.createElement("video");

      video.srcObject = stream.get_media_stream();
      document.body.appendChild(video);
      video.play();
    });
  });
  caller.on_local_stream(function (stream, error) {
    if (stream) {
      var video = document.createElement("video");

      video.srcObject = stream.get_media_stream();
      document.body.appendChild(video);
      video.play();
    } else {
      console.log(error);
    }
  });

  responder.on_local_stream(function (stream, error) {
    if (stream) {
      var video = document.createElement("video");

      video.srcObject = stream.get_media_stream();
      document.body.appendChild(video);
      video.play();
    } else {
      console.log(error);
    }
  });
  responder_room.on_new_connection(function (connection) {
    console.log("responder got new connection with member " + connection.member_id());
    connection.on_remote_stream(function (stream) {
      console.log("got video from remote member " + connection.member_id());

      var video = document.createElement("video");

      video.srcObject = stream.get_media_stream();
      document.body.appendChild(video);
      video.play();
    });
  });

  return {
    caller: caller_room,
    responder: responder_room,
    test: {
      helloWorld: function () {
        console.log("Hello world!");
      }
    }
    }
}


window.f = f;
