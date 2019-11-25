const controlUrl = "http://127.0.0.1:8000/control-api/";
const baseUrl = 'ws://127.0.0.1:8080/ws/';

let roomId = window.location.hash.replace("#", "");

async function createRoom(roomId, memberId) {
  let resp = await axios({
    method: 'post',
    url: controlUrl + roomId,
    data: {
      kind: 'Room',
      pipeline: {
        [memberId]: {
          kind: 'Member',
          credentials: 'test',
          pipeline: {
            publish: {
              kind: 'WebRtcPublishEndpoint',
              p2p: 'Always'
            },
          }
        }
      }
    }
  });

  return resp.data.sids[memberId]
}

async function createMember(roomId, memberId) {
  let controlRoom = await axios.get(controlUrl + roomId);
  let anotherMembers = Object.keys(controlRoom.data.element.pipeline);
  let pipeline = {
    publish: {
      kind: 'WebRtcPublishEndpoint',
      p2p: 'Always'
    }
  };

  let memberIds = [];

  for (let i = 0; i < anotherMembers.length; i++) {
    let memberId = anotherMembers[i];
    memberIds.push(memberId);
    pipeline["play-" + memberId] = {
      kind: 'WebRtcPlayEndpoint',
      src: 'local://' + roomId + '/' + memberId + "/publish"
    }
  }

  let resp = await axios({
    method: 'post',
    url: controlUrl + roomId + '/' + memberId,
    data: {
      kind: 'Member',
      credentials: 'test',
      pipeline: pipeline,
    }
  });

  try {
    for (let i = 0; i < memberIds.length; i++) {
      let id = memberIds[i];
      await axios({
        method: 'post',
        url: controlUrl + roomId + "/" + id + '/' + 'play-' + memberId,
        data: {
          kind: 'WebRtcPlayEndpoint',
          src: 'local://' + roomId + '/' + memberId + '/publish'
        }
      })
    }

  } catch (e) {
    console.log(e.response);
  }

  return resp.data.sids[memberId]
}

const colorizedJson = {
  replacer: function(match, pIndent, pKey, pVal, pEnd) {
    let key = '<span class=json__key>';
    let val = '<span class=json__value>';
    let str = '<span class=json__string>';
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
    let container = document.getElementsByClassName('control-debug__window_create-endpoint')[0];
    bindCloseWindow(container);

    let publishEndpointSpecContainer = container.getElementsByClassName('webrtc-publish-endpoint-spec')[0];
    let playEndpointSpecContainer = container.getElementsByClassName('webrtc-play-endpoint-spec')[0];

    let endpointTypeSelect = container.getElementsByClassName('control-debug__endpoint-type')[0];
    endpointTypeSelect.addEventListener('change', () => {
      switch (endpointTypeSelect.value) {
        case 'WebRtcPlayEndpoint':
          contentVisibility.show(playEndpointSpecContainer);
          contentVisibility.hide(publishEndpointSpecContainer);
          break;
        case 'WebRtcPublishEndpoint':
          contentVisibility.show(publishEndpointSpecContainer);
          contentVisibility.hide(playEndpointSpecContainer);
          break;
      }
    });


    let execute = container.getElementsByClassName('control-debug__execute')[0];
    execute.addEventListener('click', async () => {
      let roomId = container.getElementsByClassName('control-debug__id_room')[0].value;
      let memberId = container.getElementsByClassName('control-debug__id_member')[0].value;
      let endpointId = container.getElementsByClassName('control-debug__id_endpoint')[0].value;
      let endpointType = container.getElementsByClassName('control-debug__endpoint-type')[0].value;
      switch (endpointType) {
        case 'WebRtcPublishEndpoint':
          let p2pMode = container.getElementsByClassName('webrtc-publish-endpoint-spec__p2p')[0].value;
          await controlApi.createEndpoint(roomId, memberId, endpointId, {
            kind: endpointType,
            p2p: p2pMode,
          });
          break;
        case 'WebRtcPlayEndpoint':
          let source = container.getElementsByClassName('webrtc-play-endpoint-spec__src')[0].value;
          await controlApi.createEndpoint(roomId, memberId, endpointId, {
            kind: endpointType,
            src: source,
          });
      }
    })
  },

  delete: function() {
    let container = document.getElementsByClassName('control-debug__window_delete')[0];
    bindCloseWindow(container);

    let execute = container.getElementsByClassName('control-debug__execute')[0];
    execute.addEventListener('click', async () => {
      let roomId = container.getElementsByClassName('control-debug__id_room')[0].value;
      let memberId = container.getElementsByClassName('control-debug__id_member')[0].value;
      let endpointId = container.getElementsByClassName('control-debug__id_endpoint')[0].value;
      await controlApi.delete(roomId, memberId, endpointId);
    });
  },

  createRoom: function() {
    let container = document.getElementsByClassName('control-debug__window_create-room')[0];

    bindCloseWindow(container);

    let execute = container.getElementsByClassName('control-debug__execute')[0];
    execute.addEventListener('click', async () => {
      let roomId = container.getElementsByClassName('control-debug__id_room')[0].value;

      await controlApi.createRoom(roomId);
    });
  },

  createMember: function() {
    let container = document.getElementsByClassName('control-debug__window_create-member')[0];
    bindCloseWindow(container);

    let execute = container.getElementsByClassName('control-debug__execute')[0];
    execute.addEventListener('click', async () => {
      let roomId = container.getElementsByClassName('control-debug__id_room')[0].value;
      let memberId = container.getElementsByClassName('control-debug__id_member')[0].value;
      let credentials = container.getElementsByClassName('member-spec__credentials')[0].value;

      await controlApi.createMember(roomId, memberId, credentials);
    });
  },

  get: function() {
    let container = document.getElementsByClassName('control-debug__window_get')[0];
    let resultContainer = container.getElementsByClassName('control-debug__json-result')[0];
    bindCloseWindow(container);

    let execute = container.getElementsByClassName('control-debug__execute')[0];
    execute.addEventListener('click', async () => {
      let roomId = container.getElementsByClassName('control-debug__id_room')[0].value;
      let memberId = container.getElementsByClassName('control-debug__id_member')[0].value;
      let endpointId = container.getElementsByClassName('control-debug__id_endpoint')[0].value;

      let res = await controlApi.get(roomId, memberId, endpointId);
      resultContainer.innerHTML = colorizedJson.prettyPrint(res);
    })
  },
};

window.onload = async function() {
  let rust = await import("../../pkg");
  let jason = new rust.Jason();
  console.log(baseUrl);

  Object.values(controlDebugWindows).forEach(s => s());

  bindControlDebugMenu();

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

  async function getStream(audio_select, video_select) {
    let constraints = new rust.MediaStreamConstraints();
    let audio = new rust.AudioTrackConstraints();
    let audioSource = audio_select.options[audio_select.selectedIndex];
    if (audioSource) {
      audio.device_id(audioSource.value);
    }
    constraints.audio(audio);
    let video = new rust.VideoTrackConstraints();
    let videoSource = video_select.options[video_select.selectedIndex];
    if (videoSource) {
      video.device_id(videoSource.value);
    }
    constraints.video(video);
    return await jason.media_manager().init_local_stream(constraints);
  }

  try {
    let controlBtns = document.getElementsByClassName('control')[0];
    let joinCallerButton = document.getElementsByClassName('connect__join')[0];
    let usernameInput = document.getElementsByClassName('connect__username')[0];
    let audioSelect = document.getElementsByClassName('connect__select-device_audio')[0];
    let videoSelect = document.getElementsByClassName('connect__select-device_video')[0];
    let localVideo = document.querySelector('.local-video > video');

    const updateLocalVideo = async (stream) => {
      localVideo.srcObject = stream;
      await localVideo.play();
    };

    const room = await jason.init_room();
    try {
      const stream = await getStream(audioSelect, videoSelect);
      await updateLocalVideo(stream);
      await fillMediaDevicesInputs(audioSelect, videoSelect, stream);
      room.inject_local_stream(stream);
    } catch (e) {
      console.error("Init local video failed: " + e.message());
    }

    audioSelect.addEventListener('change', async () => {
      try {
        const stream = await getStream(audioSelect, videoSelect);
        await updateLocalVideo(stream);
        room.inject_local_stream(stream);
      } catch (e) {
        console.error("Changing audio source failed: " + e);
      }
    });

    videoSelect.addEventListener('change', async () => {
      try {
        const stream = await getStream(audioSelect, videoSelect);
        await updateLocalVideo(stream);
        room.inject_local_stream(stream);
      } catch (e) {
        console.error("Changing video source failed: " + e);
      }
    });

    room.on_new_connection( (connection) => {
      connection.on_remote_stream( async (stream) => {
        let videoDiv = document.getElementsByClassName("remote-videos")[0];
        let video = document.createElement("video");
        video.srcObject = stream.get_media_stream();
        let innerVideoDiv = document.createElement("div");
        innerVideoDiv.className = "video";
        innerVideoDiv.appendChild(video);
        videoDiv.appendChild(innerVideoDiv);

        await video.play();
      });
    });

    room.on_failed_local_stream((error) => {
      console.error(error);
    });

    let muteAudio = document.getElementsByClassName('control__mute_audio')[0];
    let muteVideo = document.getElementsByClassName('control__mute_video')[0];
    let isAudioMuted = false;
    let isVideoMuted = false;

    muteAudio.addEventListener('click', () => {
      if (isAudioMuted) {
        room.unmute_audio();
        isAudioMuted = false;
        muteAudio.textContent = "Mute audio";
      } else {
        room.mute_audio();
        isAudioMuted = true;
        muteAudio.textContent = "Unmute audio";
      }
    });
    muteVideo.addEventListener('click', () => {
      if (isVideoMuted) {
        room.unmute_video();
        isVideoMuted = false;
        muteVideo.textContent = "Mute video";
      } else {
        room.mute_video();
        isVideoMuted = true;
        muteVideo.textContent = "Unmute video";
      }
    });

    usernameInput.value = faker.name.firstName();

    let bindJoinButtons = function(roomId) {
      joinCallerButton.onclick = async function() {
        contentVisibility.show(controlBtns);

        try {
          let username = usernameInput.value;
          try {
            await axios.get(controlUrl + roomId);
          } catch (e) {
            if (e.response.status === 400) {
              console.log("Room not found. Creating new room...");
              await room.join(await createRoom(roomId, username));
              return;
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

const contentVisibility = {
  show: function(elem) {
    elem.classList.add('is-visible');
  },

  hide: function(elem) {
    elem.classList.remove('is-visible');
  },

  toggle: function(elem) {
    elem.classList.toggle('is-visible');
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

  createMember: async function(roomId, memberId, credentials) {
    try {
      await axios({
        method: 'post',
        url: controlUrl + roomId + '/' + memberId,
        data: {
          kind: 'Member',
          credentials: credentials,
          pipeline: {}
        }
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
    } catch (e) {
      alert(JSON.stringify(e.response.data));
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
  }
};

function bindCloseWindow(container) {
  container.getElementsByClassName('window__close')[0].addEventListener('click', () => {
    contentVisibility.hide(container);
  });
}

const debugMenuItems = [
  'create-endpoint',
  'create-member',
  'create-room',
  'delete',
  'get',
];

function bindControlDebugMenu() {
  let menuToggle = document.getElementsByClassName('control-debug-menu__toggle')[0];
  let menuContainer = document.getElementsByClassName('control-debug-menu')[0];
  menuToggle.addEventListener('click', () => {
    contentVisibility.toggle(menuContainer);
  });

  for (let i = 0; i < debugMenuItems.length; i++) {
    let currentItem = debugMenuItems[i];
    let currentMenuItem = menuContainer.getElementsByClassName('control-debug-menu__item_' + currentItem)[0];
    currentMenuItem.addEventListener('click', () => {
      for (let a = 0; a < debugMenuItems.length; a++) {
        if (a === i) {
          continue;
        }
        let hideContainer = document.getElementsByClassName('control-debug__window_' + debugMenuItems[a])[0];
        contentVisibility.hide(hideContainer);
      }
      let currentContainer = document.getElementsByClassName('control-debug__window_' + currentItem)[0];
      contentVisibility.show(currentContainer);
    });
  }
}
