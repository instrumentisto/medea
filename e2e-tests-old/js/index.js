/**
 * Start Pub<=>Pub video call.
 *
 * This function is used in all tests from "Pub<=>Pub video call" context.
 *
 * @returns {Promise<{caller, responder}>} promise return created caller's and receiver's rooms
 */
async function startPubPubVideoCall() {
  const rust = await import("../../jason/pkg");

  let caller = new rust.Jason();
  let responder = new rust.Jason();

  let caller_room = await caller.join_room("ws://localhost:8080/ws/pub-pub-e2e-call/caller/test");
  let responder_room = await responder.join_room("ws://localhost:8080/ws/pub-pub-e2e-call/responder/test");

  caller_room.on_new_connection(function(connection) {
    console.log("caller got new connection with member " + connection.member_id());
    connection.on_remote_stream(function(stream) {
      console.log("got video from remote member " + connection.member_id());

      let video = document.createElement("video");
      video.id = 'callers-partner-video';

      video.srcObject = stream.get_media_stream();
      document.body.appendChild(video);
      video.play();
    });
  });
  caller.on_local_stream(function(stream, error) {
    if (stream) {
      let video = document.createElement("video");

      video.srcObject = stream.get_media_stream();
      document.body.appendChild(video);
      video.play();
    } else {
      console.log(error);
    }
  });

  responder.on_local_stream(function(stream, error) {
    if (stream) {
      let video = document.createElement("video");

      video.srcObject = stream.get_media_stream();
      document.body.appendChild(video);
      video.play();
    } else {
      console.log(error);
    }
  });
  responder_room.on_new_connection(function(connection) {
    console.log("responder got new connection with member " + connection.member_id());
    connection.on_remote_stream(function(stream) {
      console.log("got video from remote member " + connection.member_id());

      let video = document.createElement("video");
      video.id = 'responders-partner-video';

      video.srcObject = stream.get_media_stream();
      document.body.appendChild(video);
      video.play();
    });
  });

  return {
    caller: caller_room,
    responder: responder_room,
  }
}

window.startPubPubVideoCall = startPubPubVideoCall;
