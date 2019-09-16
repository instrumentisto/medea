async function getDevices(participant, audio_select, video_select) {
    const device_infos = await participant.media_manager().enumerate_devices();
    console.log('Available input and output devices:', device_infos);
    for (const device_info of device_infos) {
        const option = document.createElement('option');
        option.value = device_info.device_id();
        if (device_info.kind() === 'audio') {
            option.text = device_info.label() || `Microphone ${audio_select.length + 1}`;
            audio_select.append(option);
        } else if (device_info.kind() === 'video') {
            option.text = device_info.label() || `Camera ${video_select.length + 1}`;
            video_select.append(option);
        }
    }
}

async function getStream(participant, local_video, audio_select, video_select) {
    const audio_source = audio_select.value ? {deviceId: {exact: audio_select.value}} : true;
    const video_source = video_select.value ? {deviceId: {exact: video_select.value}} : true;
    const constraints = {
        audio: audio_source,
        video: video_source
    };
    let stream = await participant.media_manager().init_local_stream(constraints);
    local_video.srcObject = stream;
    local_video.play();
}

async function init_participant(wasm, token, frame) {
    let toggle_audio = $(frame).find("input[name=toggle-audio]");
    let toggle_video = $(frame).find("input[name=toggle-video]");
    let local_video = $(frame).find("video[name=local-video]")[0];
    let remote_video = $(frame).find("video[name=remote-video]")[0];
    let audio_select = $(frame).find("select[name=audio-source]");
    let video_select = $(frame).find("select[name=video-source]");
    let join_button = $(frame).find("button[name=join-room]");

    let participant = new wasm.Jason();
    let room = await participant.init_room();
    await getStream(participant, local_video, audio_select, video_select);
    getDevices(participant, audio_select, video_select);

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

    audio_select.change(function() {
        getStream(participant, local_video, audio_select, video_select);
    });

    video_select.change(function() {
        getStream(participant, local_video, audio_select, video_select);
    });

    room.on_new_connection(function (connection) {
        connection.on_remote_stream(function (stream) {
            remote_video.srcObject = stream.get_media_stream();
            remote_video.play();
        });
    });

    participant.on_local_stream(function (stream, error) {
        if (stream) {
            audio_select.prop( "disabled", true );
            video_select.prop( "disabled", true );
            local_video.srcObject = stream.get_media_stream();
            local_video.play();
        } else {
            console.log(error);
        }
    });

    join_button.click(function() {
        room.join(token);
        join_button.prop( "disabled", true );
    });

    return room;
}

window.onload = async function () {

    const wasm = await import("../../pkg");

    await init_participant(wasm, "ws://localhost:8080/ws/pub-pub-video-call/caller/test", "#caller");
    await init_participant(wasm, "ws://localhost:8080/ws/pub-pub-video-call/responder/test", "#responder");

};

