async function init_participant(wasm, token, frame) {
  let toggle_audio = $(frame).find("input[name=toggle-audio]");
  let toggle_video = $(frame).find("input[name=toggle-video]");
  let local_video = $(frame).find("video[name=local-video]")[0];
  let remote_video = $(frame).find("video[name=remote-video]")[0];

  let participant = new wasm.Jason();
  let room = await participant.join_room(token);

  toggle_audio.change(function() {
    if($(this).is(":checked")) {
      room.unmute_audio();
    } else {
      room.mute_audio();
    }
  });

  toggle_video.change(function() {
    if($(this).is(":checked")) {
      room.unmute_video();
    } else {
      room.mute_video();
    }
  });

  room.on_new_connection(function (connection) {
    connection.on_remote_stream(function (stream) {
      remote_video.srcObject = stream.get_media_stream();
      remote_video.play();
    });
  });

  participant.on_local_stream(function (stream, error) {
    if (stream) {
      local_video.srcObject = stream.get_media_stream();
      local_video.play();
    } else {
      console.log(error);
    }
  });

  return room;
}

window.onload = async function () {
  const wasm = await import("../../pkg");

  await init_participant(wasm, "ws://localhost:8080/ws/pub-pub-video-call/caller/test", "#caller");
  await init_participant(wasm, "ws://localhost:8080/ws/pub-pub-video-call/responder/test", "#responder");
};
