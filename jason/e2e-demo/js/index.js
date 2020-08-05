const controlDomain = 'http://127.0.0.1:8000';
const controlUrl = controlDomain + '/control-api/';
const baseUrl = 'ws://127.0.0.1:8080/ws/';

let roomId = window.location.hash.replace("#", "");
let remote_videos = {};

let usernameInput = document.getElementsByClassName('connection-settings__username')[0];
let usernameMenuButton = document.getElementById('username-menu-button');

function getMemberId() {
  return usernameInput.value;
}

async function createRoom(roomId, memberId) {
  let isAudioEnabled = document.getElementById('connection-settings__publish_audio').checked;
  let isVideoEnabled = document.getElementById('connection-settings__publish_video').checked;
  let isPublish = document.getElementById('connection-settings__publish_is-enabled').checked;
  let audioPublishPolicy;
  let videoPublishPolicy;
  if (isAudioEnabled) {
    audioPublishPolicy = 'Optional';
  } else {
    audioPublishPolicy = 'Disabled';
  }
  if (isVideoEnabled) {
    videoPublishPolicy = 'Optional';
  } else {
    videoPublishPolicy = 'Disabled';
  }

  let pipeline = {};
  if (isPublish) {
    pipeline["publish"] = {
      kind: 'WebRtcPublishEndpoint',
      p2p: 'Always',
      force_relay: false,
      audio_settings: {
        publish_policy: audioPublishPolicy,
      },
      video_settings: {
        publish_policy: videoPublishPolicy,
      }
    };
  }

  let resp = await axios({
    method: 'post',
    url: controlUrl + roomId,
    data: {
      kind: 'Room',
      pipeline: {
        [memberId]: {
          kind: 'Member',
          credentials: 'test',
          pipeline: pipeline,
          on_join: "grpc://127.0.0.1:9099",
          on_leave: "grpc://127.0.0.1:9099"
        }
      }
    }
  });

  return resp.data.sids[memberId]
}

async function createMember(roomId, memberId) {
  let isAudioEnabled = document.getElementById('connection-settings__publish_audio').checked;
  let isVideoEnabled = document.getElementById('connection-settings__publish_video').checked;
  let audioPublishPolicy;
  let videoPublishPolicy;
  if (isAudioEnabled) {
    audioPublishPolicy = 'Optional';
  } else {
    audioPublishPolicy = 'Disabled';
  }
  if (isVideoEnabled) {
    videoPublishPolicy = 'Optional';
  } else {
    videoPublishPolicy = 'Disabled';
  }
  let isPublish = document.getElementById('connection-settings__publish_is-enabled').checked;

  let controlRoom = await axios.get(controlUrl + roomId);
  let anotherMembers = Object.values(controlRoom.data.element.pipeline);
  let pipeline = {};

  let memberIds = [];
  if (isPublish) {
    pipeline["publish"] = {
      kind: 'WebRtcPublishEndpoint',
      p2p: 'Always',
      force_relay: false,
      audio_settings: {
        publish_policy: audioPublishPolicy,
      },
      video_settings: {
        publish_policy: videoPublishPolicy,
      },
    };
  }
  for (let i = 0; i < anotherMembers.length; i++) {
    let anotherMember = anotherMembers[i];
    let memberId = anotherMember.id;
    memberIds.push(memberId);
    if (anotherMember.pipeline.hasOwnProperty('publish')) {
      pipeline["play-" + memberId] = {
        kind: 'WebRtcPlayEndpoint',
        src: 'local://' + roomId + '/' + memberId + "/publish",
        force_relay: false
      }
    }
  }

  let resp = await axios({
    method: 'post',
    url: controlUrl + roomId + '/' + memberId,
    data: {
      kind: 'Member',
      credentials: 'test',
      pipeline: pipeline,
      on_join: "grpc://127.0.0.1:9099",
      on_leave: "grpc://127.0.0.1:9099"
    }
  });

  if (isPublish) {
    try {
      for (let i = 0; i < memberIds.length; i++) {
        let id = memberIds[i];
        await axios({
          method: 'post',
          url: controlUrl + roomId + "/" + id + '/' + 'play-' + memberId,
          data: {
            kind: 'WebRtcPlayEndpoint',
            src: 'local://' + roomId + '/' + memberId + '/publish',
            force_relay: false
          }
        })
      }

    } catch (e) {
      console.log(e.response);
    }
  }

  return resp.data.sids[memberId];
}

const colorizedJson = {
  replacer: function(match, pIndent, pKey, pVal, pEnd) {
    let key = '<span class="json__key">';
    let val = '<span class="json__value">';
    let str = '<span class="json__string">';
    let r = pIndent || '';
    if (pKey)
      r = r + key + pKey.replace(/[": ]/g, '') + '</span>: ';
    if (pVal)
      r = r + (pVal[0] === '"' ? str : val) + pVal + '</span>';
    return r + (pEnd || '');
  },

  prettyPrint: function(obj) {
    let jsonLine = /^( *)("[\w\-]+": )?("[^"]*"|[\w.+-]*)?([,[{])?$/mg;
    return JSON.stringify(obj, null, 3)
      .replace(/&/g, '&amp;').replace(/\\"/g, '&quot;')
      .replace(/</g, '&lt;').replace(/>/g, '&gt;')
      .replace(jsonLine, colorizedJson.replacer);
  }
};

const controlDebugWindows = {
  createEndpoint: function() {
    let container = document.getElementById('control-debug__window_create_endpoint');

    let publishEndpointSpecContainer = container.getElementsByClassName('webrtc-publish-endpoint-spec')[0];
    let playEndpointSpecContainer = container.getElementsByClassName('webrtc-play-endpoint-spec')[0];

    let endpointTypeSelect = container.getElementsByClassName('control-debug__endpoint-type')[0];
    endpointTypeSelect.addEventListener('change', () => {
      switch (endpointTypeSelect.value) {
        case 'WebRtcPlayEndpoint':
          $( playEndpointSpecContainer ).show();
          $( publishEndpointSpecContainer ).hide();
          break;
        case 'WebRtcPublishEndpoint':
          $( publishEndpointSpecContainer ).show();
          $( playEndpointSpecContainer ).hide();
          break;
      }
    });


    let execute = container.getElementsByClassName('control-debug__execute')[0];
    execute.addEventListener('click', async () => {
      let roomId = container.getElementsByClassName('control-debug__id_room')[0].value;
      let memberId = container.getElementsByClassName('control-debug__id_member')[0].value;
      let endpointId = container.getElementsByClassName('control-debug__id_endpoint')[0].value;
      let endpointType = container.getElementsByClassName('control-debug__endpoint-type')[0].value;
      if (endpointType === 'WebRtcPublishEndpoint') {
          let p2pMode = container.getElementsByClassName('webrtc-publish-endpoint-spec__p2p')[0].value;
          let isForceRelay = document.getElementById('webrtc-publish-endpoint-spec__force-relay').checked;
          let audioPublishPolicy = document.getElementsByClassName('webrtc-publish-endpoint-spec__publish-policy_audio')[0].value;
          let videoPublishPolicy = document.getElementsByClassName('webrtc-publish-endpoint-spec__publish-policy_video')[0].value;
          await controlApi.createEndpoint(roomId, memberId, endpointId, {
            kind: endpointType,
            p2p: p2pMode,
            force_relay: isForceRelay,
            audio_settings: {
              publish_policy: audioPublishPolicy,
            },
            video_settings: {
              publish_policy: videoPublishPolicy,
            },
          });
      } else if (endpointType === 'WebRtcPlayEndpoint') {
          let source = 'local://' + container.getElementsByClassName('webrtc-play-endpoint-spec__src')[0].value;
          let isForceRelay = document.getElementById('webrtc-play-endpoint-spec__force-relay').checked;
          await controlApi.createEndpoint(roomId, memberId, endpointId, {
            kind: endpointType,
            src: source,
            force_relay: isForceRelay,
          });
      }
    })
  },

  delete: function() {
    let container = document.getElementById('control-debug__window_delete');

    let execute = container.getElementsByClassName('control-debug__execute')[0];
    execute.addEventListener('click', async () => {
      let roomId = container.getElementsByClassName('control-debug__id_room')[0].value;
      let memberId = container.getElementsByClassName('control-debug__id_member')[0].value;
      let endpointId = container.getElementsByClassName('control-debug__id_endpoint')[0].value;
      await controlApi.delete(roomId, memberId, endpointId);
    });
  },

  createRoom: function() {
    let container = document.getElementById('control-debug__window_create_room');

    let execute = container.getElementsByClassName('control-debug__execute')[0];
    execute.addEventListener('click', async () => {
      let roomId = container.getElementsByClassName('control-debug__id_room')[0].value;

      await controlApi.createRoom(roomId);
    });
  },

  createMember: function() {
    let container = document.getElementById('control-debug__window_create_member');

    let execute = container.getElementsByClassName('control-debug__execute')[0];
    execute.addEventListener('click', async () => {
      let roomId = container.getElementsByClassName('control-debug__id_room')[0].value;
      let memberId = container.getElementsByClassName('control-debug__id_member')[0].value;
      let credentials = container.getElementsByClassName('member-spec__credentials')[0].value;

      let idleTimeout = container.getElementsByClassName('member-spec__idle-timeout')[0].value;
      let reconnectTimeout = container.getElementsByClassName('member-spec__reconnect-timeout')[0].value;
      let pingInterval = container.getElementsByClassName('member-spec__ping-interval')[0].value;

      let spec = {};
      if (credentials.length > 0) {
        spec.credentials = credentials;
      }
      if (idleTimeout.length > 0) {
        spec.idle_timeout = idleTimeout;
      }
      if (reconnectTimeout.length > 0) {
        spec.reconnect_timeout = reconnectTimeout;
      }
      if (pingInterval.length > 0) {
        spec.ping_interval = pingInterval;
      }

      await controlApi.createMember(roomId, memberId, spec);
    });
  },

  get: function() {
    let container = document.getElementById('control-debug__window_get');
    let resultContainer = container.getElementsByClassName('control-debug__json-result')[0];

    let execute = container.getElementsByClassName('control-debug__execute')[0];
    execute.addEventListener('click', async () => {
      let roomId = container.getElementsByClassName('control-debug__id_room')[0].value;
      let memberId = container.getElementsByClassName('control-debug__id_member')[0].value;
      let endpointId = container.getElementsByClassName('control-debug__id_endpoint')[0].value;

      let res = await controlApi.get(roomId, memberId, endpointId);
      resultContainer.innerHTML = colorizedJson.prettyPrint(res);
    })
  },

  callbacks: function() {
    let container = document.getElementById('control-debug__window_callbacks');
    let resultContainer = container.getElementsByClassName('control-debug__table-result')[0];

    let execute = container.getElementsByClassName('control-debug__execute')[0];
    execute.addEventListener('click', async () => {
      while (resultContainer.firstChild) {
        resultContainer.firstChild.remove();
      }

      let callbacks = await controlApi.getCallbacks();

      let table = document.createElement("table");
      table.className = "table";

      let thead = document.createElement("thead");
      let header = document.createElement("tr");
      let eventHeader = document.createElement("th");
      eventHeader.innerHTML = 'Event';
      eventHeader.scope = 'col';
      header.appendChild(eventHeader);
      let timeHeader = document.createElement("th");
      timeHeader.innerHTML = 'Time';
      timeHeader.scope = 'col';
      header.appendChild(timeHeader);
      let elementHeader = document.createElement('th');
      elementHeader.innerHTML = 'FID';
      elementHeader.scope = 'col';
      header.appendChild(elementHeader);
      thead.appendChild(header);
      table.appendChild(thead);

      let tbody = document.createElement('tbody');
      for (callback of callbacks) {
        let row = document.createElement('tr');
        let event = document.createElement('td');
        event.innerHTML = JSON.stringify(callback.event);
        row.appendChild(event);
        let time = document.createElement('td');
        time.innerHTML = callback.at;
        row.appendChild(time);
        let element = document.createElement('td');
        element.innerHTML = callback.fid;
        row.appendChild(element);
        tbody.appendChild(row);
      }

      table.appendChild(tbody);

      resultContainer.appendChild(table);
    })
  }
};

async function startPublishing() {
  let memberId = getMemberId();
  let roomSpec = await controlApi.get(roomId, '', '');
  let anotherMembers = Object.values(roomSpec.element.pipeline);
  let membersToConnect = [];
  anotherMembers.forEach((anotherMember) => {
    if (anotherMember.id != memberId) {
      membersToConnect.push(anotherMember.id);
    }
  });

  let publishEndpoint = {
    kind: 'WebRtcPublishEndpoint',
    p2p: 'Always',
  };
  let isSuccess = await controlApi.createEndpoint(roomId, memberId, 'publish', publishEndpoint);
  if (!isSuccess) {
    return;
  }

  membersToConnect.forEach(async (srcMemberId) => {
    let endpoint = {
      kind: 'WebRtcPlayEndpoint',
      src: `local://${roomId}/${memberId}/publish`,
      force_relay: false,
    };
    await controlApi.createEndpoint(roomId, srcMemberId, 'play-' + memberId, endpoint);
  });
}

window.onload = async function() {
  let rust = await import("../../pkg");
  let jason = new rust.Jason();
  console.log(baseUrl);
  usernameInput.addEventListener('change', (e) => {
    usernameMenuButton.innerHTML = e.target.value;
  });

  $('.modal').on('show.bs.modal', function(event) {
      var idx = $('.modal:visible').length;
      $(this).css('z-index', 1040 + (10 * idx));
  });
  $('.modal').on('shown.bs.modal', function(event) {
      var idx = ($('.modal:visible').length) -1; // raise backdrop after animation.
      $('.modal-backdrop').not('.stacked').css('z-index', 1039 + (10 * idx));
      $('.modal-backdrop').not('.stacked').addClass('stacked');
  });

  $('#connection-settings').modal('show');

  let startPublishingBtn = document.getElementById('enable-publishing-btn');
  startPublishingBtn.addEventListener('click', async () => {
    await startPublishing();
  });



  Object.values(controlDebugWindows).forEach(s => s());

  let room = newRoom();
  let isCallStarted = false;
  let localStream = null;
  let isAudioMuted = false;
  let isVideoMuted = false;
  let audioSelect = document.getElementsByClassName('connect__select-device_audio')[0];
  let videoSelect = document.getElementsByClassName('connect__select-device_video')[0];
  let localVideo = document.querySelector('.local-video > video');

  async function initLocalStream() {
      let constraints = await build_constraints(
        isAudioMuted ? null : audioSelect,
        isVideoMuted ? null : videoSelect
      );
      try {
        localStream = await jason.media_manager().init_local_stream(constraints)
      } catch (e) {
        let origError = e.source();
        if (origError && (origError.name === "NotReadableError" || origError.name === "AbortError")) {
          if (origError.message.includes("audio")) {
            constraints = await build_constraints(null, videoSelect);
            localStream = await jason.media_manager().init_local_stream(constraints);
            alert("unable to get audio, will try to enter room with video only");
          } else if (origError.message.includes("video")) {
            constraints = await build_constraints(audioSelect, null);
            localStream = await jason.media_manager().init_local_stream(constraints);
            alert("unable to get video, will try to enter room with audio only");
          } else {
            throw e;
          }
        } else {
          throw e;
        }
      }
      await updateLocalVideo(localStream);

      return constraints;
  }

  async function fillMediaDevicesInputs(audio_select, video_select, current_stream) {
    const current_audio = (current_stream.getAudioTracks().pop() || { label: "disable" }).label || "disable";
    const current_video = (current_stream.getVideoTracks().pop() || { label: "disable" }).label || "disable";
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
    const option = document.createElement('option');
    option.value = "screen";
    option.text = "screen";
    video_select.append(option);
  }

  async function build_constraints(audio_select, video_select) {
    let constraints = new rust.MediaStreamSettings();
    if (audio_select != null) {
      let audio = new rust.AudioTrackConstraints();
      let audioSource = audio_select.options[audio_select.selectedIndex];
      if (audioSource) {
        audio.device_id(audioSource.value);
      }
      constraints.audio(audio);
    }

    if (video_select != null) {
      let videoSource = video_select.options[video_select.selectedIndex];
      if (videoSource) {
        if (videoSource.value === "screen") {
          let video = new rust.DisplayVideoTrackConstraints();
          constraints.display_video(video);
        } else {
          let video = new rust.DeviceVideoTrackConstraints();
          video.device_id(videoSource.value);
          constraints.device_video(video);
        }
      } else {
        constraints.device_video(new rust.DeviceVideoTrackConstraints());
      }
    }

    return constraints;
  }

  const updateLocalVideo = async (stream) => {
    localVideo.srcObject = stream.get_media_stream();
    await localVideo.play();
  };

  async function newRoom() {
    jason = new rust.Jason();
    room = await jason.init_room();

    try {
      const constraints = await initLocalStream();
      await fillMediaDevicesInputs(audioSelect, videoSelect, localStream.get_media_stream());
      await room.set_local_media_settings(constraints);
    } catch (e) {
      console.error("Init local video failed: " + e);
    }

    room.on_quality_score_update((update) => {
      console.log(update.avg_quality_score());
      for (const [key, value] of update.quality_scores().entries()) {
        console.log(`${key}: ${value}`);
      }
    });

    room.on_new_connection( (connection) => {
      isCallStarted = true;
      connection.on_remote_stream( async (stream) => {
        let videoDiv = document.getElementsByClassName("remote-videos")[0];
        let video = document.createElement("video");
        video.controls = "true";
        video.srcObject = stream.get_media_stream();
        let innerVideoDiv = document.createElement("div");
        innerVideoDiv.className = "video";
        innerVideoDiv.appendChild(video);
        remote_videos[connection.get_remote_id()] = innerVideoDiv;
        videoDiv.appendChild(innerVideoDiv);

        video.oncanplay = async () => {
          await video.play();
        };
        stream.on_track_added( (track) => {
          // switch controls off and on, cause controls are not updated
          // automatically
          let video = remote_videos[connection.get_remote_id()]
            .getElementsByTagName("video")[0];
          video.controls = false;
          video.controls = true;
          console.log(`Track added: ${track.kind}`);
          console.log(`Has active audio: ${stream.has_active_audio()}`);
          console.log(`Has active video: ${stream.has_active_video()}`);
        });
        stream.on_track_enabled( (track) => {
          console.log(`Track enabled: ${track.kind}`);
          console.log(`Has active audio: ${stream.has_active_audio()}`);
          console.log(`Has active video: ${stream.has_active_video()}`);
        });
        stream.on_track_disabled( (track) => {
          console.log(`Track disabled: ${track.kind}`);
          console.log(`Has active audio: ${stream.has_active_audio()}`);
          console.log(`Has active video: ${stream.has_active_video()}`);
        });
      });

      connection.on_close(() => {
        remote_videos[connection.get_remote_id()].remove();
        delete remote_videos[connection.get_remote_id()];
      });
    });

    room.on_local_stream((stream) => {
      updateLocalVideo(stream);
      stream.free();
    });

    room.on_failed_local_stream((error) => {
      console.error(error.message());
    });

    room.on_connection_loss( async (reconnectHandle) => {
      let connectionLossNotification = document.getElementsByClassName('connection-loss-notification')[0];
      $( connectionLossNotification ).toast('show');

      let manualReconnectBtn = document.getElementsByClassName('connection-loss-notification__manual-reconnect')[0];
      let connectionLossMsg = document.getElementsByClassName('connection-loss-notification__msg')[0];
      let connectionLossDefaultText = connectionLossMsg.textContent;

      manualReconnectBtn.onclick = async () => {
        try {
          connectionLossMsg.textContent = 'Trying to manually reconnect...';
          await reconnectHandle.reconnect_with_delay(0);
          $( connectionLossNotification ).toast('hide');
          console.log("Reconnected!");
        } catch (e) {
          console.error("Failed to manually reconnect: " + e.message());
        } finally {
          connectionLossMsg.textContent = connectionLossDefaultText;
        }
      };
      try {
        await reconnectHandle.reconnect_with_backoff(3000, 2.0, 10000);
      } catch (e) {
        console.error('Error in reconnection with backoff:\n' + e.message());
      }
      $( connectionLossNotification ).toast('hide');
    });

    room.on_close(function (on_closed) {
      let videos = document.getElementsByClassName('remote-videos')[0];
      while (videos.firstChild) {
        videos.firstChild.remove();
      }
      room = newRoom();
      $('#connection-settings').modal('show');
      $('#connect-btn').show();
      $('.control').hide();
      alert(
        `Call was ended.
        Reason: ${on_closed.reason()};
        Is closed by server: ${on_closed.is_closed_by_server()};
        Is error: ${on_closed.is_err()}.`
      );
    });
  }

  try {
    let joinCallerButton = document.getElementsByClassName('connection-settings__connect')[0];

    audioSelect.addEventListener('change', async () => {
      try {
        let constraints = await build_constraints(audioSelect, videoSelect);
        if (localStream && localStream.ptr > 0 ){
          localStream.free();
        }
        if (!isAudioMuted) {
          constraints = await initLocalStream();
        }
        await room.set_local_media_settings(constraints);
      } catch (e) {
        console.error("Changing audio source failed: " + e);
      }
    });

    videoSelect.addEventListener('change', async () => {
      try {
        let constraints = await build_constraints(audioSelect, videoSelect);
        if (localStream && localStream.ptr > 0 ){
          localStream.free();
        }
        if (!isVideoMuted) {
          constraints = await initLocalStream();
        }
        await room.set_local_media_settings(constraints);
      } catch (e) {
        console.error("Changing video source failed: " + e);
      }
    });

    let muteAudio = document.getElementsByClassName('control__mute_audio')[0];
    let muteVideo = document.getElementsByClassName('control__mute_video')[0];
    let closeApp = document.getElementsByClassName('control__close_app')[0];

    muteAudio.addEventListener('click', async () => {
      try {
        if (isAudioMuted) {
          await room.unmute_audio();
          isAudioMuted = false;
          muteAudio.textContent = "Mute audio";
          if (!isCallStarted) {
            await initLocalStream();
          }
        } else {
          await room.mute_audio();
          if (localStream && localStream.ptr > 0 ){
            localStream.free_audio();
          }
          isAudioMuted = true;
          muteAudio.textContent = "Unmute audio";
        }
      } catch (e) {
        console.error(e.message());
      }
    });
    muteVideo.addEventListener('click', async () => {
      try {
        if (isVideoMuted) {
          await room.unmute_video();
          isVideoMuted = false;
          muteVideo.textContent = "Mute video";
          if (!isCallStarted) {
            await initLocalStream();
          }
        } else {
          await room.mute_video();
          if (localStream && localStream.ptr > 0 ){
            localStream.free_video();
          }
          isVideoMuted = true;
          muteVideo.textContent = "Unmute video";
        }
      } catch (e) {
        console.error(e.message());
      }
    });
    closeApp.addEventListener('click', () => {
      jason.dispose();
    });

    usernameInput.value = faker.name.firstName();
    usernameMenuButton.innerHTML = usernameInput.value;

    let bindJoinButtons = function(roomId) {
      joinCallerButton.onclick = async function() {
        $('#connection-settings').modal('hide');
        $('.control').show();
        $('#connect-btn').hide();

        try {
          let username = usernameInput.value;
          try {
            await axios.get(controlUrl + roomId);
          } catch (e) {
            if (e.response.status === 400) {
              console.log("Room not found. Creating new room...");
              await room.join(await createRoom(roomId, username));
              return;
            } else {
              throw e;
            }
          }
          try {
            await axios.get(controlUrl + roomId + '/' + username);
          } catch (e) {
            console.log("Member not found. Creating new member...");
            await room.join(await createMember(roomId, username));
            return;
          }
          await room.join(baseUrl + roomId + '/' + username + '/test')
        } catch (e) {
          console.error(e);
          console.error(
            "Join to room failed: Error[name:[", e.name(), "], ",
            "[msg:", e.message(), "], [source", e.source(), "]]",
          );
          console.error(e.trace());
        }
      };
    };

    bindJoinButtons(roomId);
  } catch (e) {
    console.log(e)
  }
};

const controlApi = {
  createRoom: async function(roomId) {
    try {
      await axios({
        method: 'post',
        url: controlUrl + roomId,
        data: {
          kind: 'Room',
          pipeline: {}
        }
      });
    } catch (e) {
      alert(JSON.stringify(e.response.data));
    }
  },

  createMember: async function(roomId, memberId, spec) {
    spec.kind = 'Member';
    spec.pipeline = {};

    try {
      await axios({
        method: 'post',
        url: controlUrl + roomId + '/' + memberId,
        data: spec
      });
    } catch (e) {
      alert(JSON.stringify(e.response.data));
    }
  },

  createEndpoint: async function(roomId, memberId, endpointId, spec) {
    try {
      await axios({
        method: 'post',
        url: controlUrl + roomId + '/' + memberId + '/' + endpointId,
        data: spec
      });

      return true;
    } catch (e) {
      alert(JSON.stringify(e.response.data));

      return false;
    }
  },

  getUrlForElement: function(roomId, memberId, endpointId) {
    let url = controlUrl + roomId;
    if (memberId.length > 0 && endpointId.length > 0) {
      url = controlUrl + roomId + '/' + memberId + '/' + endpointId;
    } else if (memberId.length > 0) {
      url = controlUrl + roomId + '/' + memberId;
    }

    return url;
  },

  delete: async function(roomId, memberId, endpointId) {
    try {
      let url = controlApi.getUrlForElement(roomId, memberId, endpointId);
      let resp = await axios.delete(url);
      return JSON.stringify(resp.data, null, 4);
    } catch (e) {
      alert(JSON.stringify(e.response.data));
    }
  },

  get: async function(roomId, memberId, endpointId) {
    try {
      let url = controlApi.getUrlForElement(roomId, memberId, endpointId);
      let resp = await axios.get(url);
      return resp.data;
    } catch (e) {
      alert(JSON.stringify(e.response.data));
    }
  },

  getCallbacks: async function() {
    try {
      let resp = await axios.get(controlDomain + '/callbacks');
      return resp.data;
    } catch (e) {
      alert(JSON.stringify(e.response.data));
    }
  }
};


