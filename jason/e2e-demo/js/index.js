function audioToggle(room, toggle) {
  if(toggle.checked) {
    room.unmute_audio();
  } else {
    room.mute_audio();
  }
}

function videoToggle(room, toggle) {
  if(toggle.checked) {
    room.unmute_video();
  } else {
    room.mute_video();
  }
}

async function init_participant(wasm, token, frameSelector) {
  let frame = document.querySelector(frameSelector);

  let toggleAudio = frame.querySelector("input[name=toggle-audio]");
  let toggleVideo = frame.querySelector("input[name=toggle-video]");
  let localVideo = frame.querySelector("video[class=local-video]");
  let remoteVideo = frame.querySelector("video[class=remote-video]");

  let participant = new wasm.Jason();
  let room = await participant.join_room(token);

  // Restore mute state after page refresh.
  audioToggle(room, toggleAudio);
  videoToggle(room, toggleVideo);

  toggleAudio.addEventListener('change', (t) => {
      audioToggle(room, t.target);
  });

  toggleVideo.addEventListener('change', (t) => {
      videoToggle(room, t.target);
  });

  room.on_new_connection(function (connection) {
    connection.on_remote_stream(function (stream) {
      remoteVideo.srcObject = stream.get_media_stream();
      remoteVideo.play();
    });
  });

  participant.on_local_stream(function (stream, error) {
    if (stream) {
      localVideo.srcObject = stream.get_media_stream();
      localVideo.play();
    } else {
      console.log(error);
    }
  });

  return room;
}

window.onload = async function () {
  const wasm = await import("../../pkg");

  await init_participant(wasm, "ws://localhost:8080/ws/pub-pub-video-call/caller/test", "#caller");
  await init_participant(wasm, "ws://localhost:8090/ws/pub-pub-video-call/responder/test", "#responder");
};
