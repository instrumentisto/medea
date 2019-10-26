async function init(){
  'use strict';
  const rust = await import('../../pkg');

  const jason = new rust.Jason();

  async function fillMediaDevicesInputs(audio_select, video_select, current_stream) {
    const current_audio = current_stream.getAudioTracks().pop().label || "disable";
    const current_video = current_stream.getVideoTracks().pop().label || "disable";
    const device_infos = await jason.media_manager().enumerate_devices();
    console.log('Available input and output devices:', device_infos);
    for (const device_info of device_infos) {
      const option = document.createElement('option');
      option.value = device_info.device_id();
      if (device_info.kind() === 'audio') {
        option.text = device_info.label() || `Microphone ${audio_select.length + 1}`;
        option.selected = option.text === current_audio;
        audio_select.append(option);
      } else if (device_info.kind() === 'video') {
        option.text = device_info.label() || `Camera ${video_select.length + 1}`;
        option.selected = option.text === current_video;
        video_select.append(option);
      }
    }
  }

  async function getStream(local_video, audio_select, video_select) {
    let constraints = new rust.MediaStreamConstraints();
    let audio = new rust.AudioTrackConstraints();
    if (audio_select.val()) {
      audio.device_id(audio_select.val())
    }
    constraints.audio(audio);
    let video = new rust.VideoTrackConstraints();
    if (video_select.val()) {
      video.device_id(video_select.val())
    }
    constraints.video(video);
    let stream = await jason.media_manager().init_local_stream(constraints);
    local_video.srcObject = stream;
    local_video.play();
    console.log(stream);
    return stream;
  }

  async function init_participant(token, frame) {
    let toggle_audio = $(frame).find("input[name=toggle-audio]");
    let toggle_video = $(frame).find("input[name=toggle-video]");
    let local_video = $(frame).find("video[name=local-video]")[0];
    let remote_video = $(frame).find("video[name=remote-video]")[0];
    let audio_select = $(frame).find("select[name=audio-source]");
    let video_select = $(frame).find("select[name=video-source]");
    let join_button = $(frame).find("button[name=join-room]");

    const room = await jason.init_room();
    const stream = await getStream(local_video, audio_select, video_select);
    await fillMediaDevicesInputs(audio_select, video_select, stream);

    toggle_audio.change(function () {
      if ($(this).is(":checked")) {
        room.unmute_audio();
      } else {
        room.mute_audio();
      }
    });

    toggle_video.change(function () {
      if ($(this).is(":checked")) {
        room.unmute_video();
      } else {
        room.mute_video();
      }
    });

    audio_select.change(async function () {
      const stream = await getStream(local_video, audio_select, video_select);
      room.inject_local_stream(stream);
    });

    video_select.change(async function () {
      const stream = await getStream(local_video, audio_select, video_select);
      room.inject_local_stream(stream);
    });

    room.on_new_connection(function (connection) {
      connection.on_remote_stream(function (stream) {
        remote_video.srcObject = stream;
        remote_video.play();
      });
    });

    room.on_local_stream(function () {
      console.log("unreachable!");
    });

    room.on_failed_local_stream(function (error) {
      console.log(error);
    });

    join_button.click(function () {
      room.join(token);
      join_button.prop("disabled", true);
    });

    return room;
  }

  return {
    init_participant: init_participant,
  };
}

window.onload = async function () {
  await init()
      .then(async medea => {
        await medea.init_participant("ws://localhost:8080/ws/pub-pub-video-call/caller/test", "#caller");
      })
      .catch(console.error);

  await init()
      .then(async medea => {
        await medea.init_participant("ws://localhost:8080/ws/pub-pub-video-call/responder/test", "#responder");
      })
      .catch(console.error);
};
