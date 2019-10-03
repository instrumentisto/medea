let roomId = window.location.hash.replace("#", "");

export async function run(credentials) {
    let wasm = await import("../../pkg");
    let jason = new wasm.Jason();

    jason.on_local_stream(function(stream, error) {
        if (stream) {
            let local_video = document.querySelector('.local-video > video');

            local_video.srcObject = stream.get_media_stream();
            local_video.play();
        } else {
            console.error(error);
        }
    });

    let room = await jason.join_room(credentials);

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

    room.on_new_connection(function(connection) {
        connection.on_remote_stream(function(stream) {
            let videoDiv = document.getElementsByClassName("remote-videos")[0];
            let video = document.createElement("video");
            video.srcObject = stream.get_media_stream();
            let innerVideoDiv = document.createElement("div");
            innerVideoDiv.className = "video";
            innerVideoDiv.appendChild(video);
            videoDiv.appendChild(innerVideoDiv);

            video.play();
        });
    });
}

window.connect_room = async function connect_room(credentials) {
    run(credentials)
};
let baseUrl = "ws://127.0.0.1:8080/ws/";
const controlUrl = "http://127.0.0.1:8000/";

async function createRoom(roomId, memberId) {
    await axios({
        method: 'post',
        url: controlUrl + roomId,
        data: {
            pipeline: {
                [memberId]: {
                    kind: 'Member',
                    credentials: 'test',
                    pipeline: {
                        publish: {
                            kind: 'WebRtcPublishEndpoint',
                            spec: {
                                p2p: 'Always'
                            }
                        },
                    }
                }
            }
        }
    });
}

async function addNewMember(roomId, memberId) {
    let controlRoom = await axios.get(controlUrl + roomId);
    let anotherMembers = Object.keys(controlRoom.data.element.pipeline);
    let pipeline = {
        publish: {
            kind: 'WebRtcPublishEndpoint',
            spec: {
                p2p: 'Always'
            }
        }
    };

    let memberIds = [];

    for (let i = 0; i < anotherMembers.length; i++) {
        let localUri = anotherMembers[i];
        let memberId = localUri.replace(/local:\/\/.*\//, "");
        memberIds.push(memberId);
        pipeline["play-" + memberId] = {
            kind: 'WebRtcPlayEndpoint',
            spec: {
                src: localUri + "/publish"
            }
        }
    }

    await axios({
        method: 'post',
        url: controlUrl + roomId + "/" + memberId,
        data: {
            credentials: 'test',
            pipeline: pipeline,
        }
    });

    for (let i = 0; i < memberIds.length; i++) {
        let id = memberIds[i];
        await axios({
            method: 'post',
            url: controlUrl + roomId + "/" + id + "/play-" + memberId,
            data: {
                kind: 'WebRtcPlayEndpoint',
                spec: {
                    src: 'local://' + roomId + '/' + memberId + '/publish'
                }
            }
        })
    }
}

const controlDebugWindows = {
    deleteRoom: function () {
        let container = document.getElementsByClassName('control-debug__window_delete-room')[0];
        bindCloseWindow(container);

        let execute = container.getElementsByClassName('control-debug__execute')[0];
        execute.addEventListener('click', async () => {
            await controlApi.deleteRoom();
        });
    },

    deleteMember: function () {
        let container = document.getElementsByClassName('control-debug__window_delete-member')[0];
        bindCloseWindow(container);
        let execute = container.getElementsByClassName('control-debug__execute')[0];
        execute.addEventListener('click', async () => {
            let roomId = container.getElementsByClassName('control-debug__id_room')[0].value;
            let memberId = container.getElementsByClassName('control-debug__id_member')[0].value;
            await controlApi.deleteMember(roomId, memberId);
        });
    },

    createEndpoint: function () {
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
                        spec: {
                            p2p: p2pMode,
                        }
                    });
                    break;
                case 'WebRtcPlayEndpoint':
                    let source = container.getElementsByClassName('webrtc-play-endpoint-spec__src')[0].value;
                    await controlApi.createEndpoint(roomId, memberId, endpointId, {
                        kind: endpointType,
                        spec: {
                            src: source,
                        }
                    });
            }
        })
    },

    deleteEndpoint: function () {
        let container = document.getElementsByClassName('control-debug__window_delete-endpoint')[0];
        bindCloseWindow(container);

        let execute = container.getElementsByClassName('control-debug__execute')[0];
        execute.addEventListener('click', async () => {
            let roomId = container.getElementsByClassName('control-debug__id_room')[0].value;
            let memberId = container.getElementsByClassName('control-debug__id_member')[0].value;
            let endpointId = container.getElementsByClassName('control-debug__id_endpoint')[0].value;
            await controlApi.deleteEndpoint(roomId, memberId, endpointId);
        });
    },

    createRoom: function () {
        let container = document.getElementsByClassName('control-debug__window_create-room')[0];

        bindCloseWindow(container);

        let execute = container.getElementsByClassName('control-debug__execute')[0];
        execute.addEventListener('click', async () => {
            let roomId = container.getElementsByClassName('control-debug__id_room')[0].value;

            await controlApi.createRoom(roomId);
        });
    },

    createMember: function () {
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

    get: function () {
        let container = document.getElementsByClassName('control-debug__window_get')[0];
        let resultContainer = container.getElementsByClassName('control-debug__json-result')[0];
        bindCloseWindow(container);

        let execute = container.getElementsByClassName('control-debug__execute')[0];
        execute.addEventListener('click', async () => {
            let roomId = container.getElementsByClassName('control-debug__id_room')[0].value;
            let memberId = container.getElementsByClassName('control-debug__id_member')[0].value;
            let endpointId = container.getElementsByClassName('control-debug__id_endpoint')[0].value;

            let res = await controlApi.get(roomId, memberId, endpointId);
            res = res.replace(/(?:\r\n|\r|\n)/g, '<br>');
            resultContainer.innerHTML = res;
        })
    },
};

window.onload = function() {
    Object.values( controlDebugWindows ).forEach( s => s() );

    bindControlDebugMenu();

    try {
        let controlBtns = document.getElementsByClassName('control')[0];
        let joinCallerButton = document.getElementsByClassName('connect__join')[0];
        let usernameInput = document.getElementsByClassName('connect__username')[0];

        usernameInput.value = faker.name.firstName();

        let bindJoinButtons = function(roomId) {
            joinCallerButton.onclick = async function() {
                let connectBtnsDiv = document.getElementsByClassName("connect")[0];
                contentVisibility.hide(connectBtnsDiv);
                contentVisibility.show(controlBtns);

                let username = usernameInput.value;
                try {
                    await axios.get(controlUrl + roomId);
                } catch (e) {
                    if (e.response.status === 400) {
                        console.log("Room not found. Creating new room...");
                        await createRoom(roomId, username);
                    }
                }
                try {
                    await axios.get(controlUrl + roomId + '/' + username);
                } catch (e) {
                    console.log("Member not found. Creating new member...");
                    await addNewMember(roomId, username);
                }
                await window.connect_room(baseUrl + roomId + '/' + username + '/test')
            };
        };

        bindJoinButtons(roomId);
    } catch (e) {
        console.log(e.response)
    }
};

const contentVisibility = {
    show: function (elem) {
        elem.classList.add('is-visible');
    },

    hide: function (elem) {
        elem.classList.remove('is-visible');
    },

    toggle: function (elem) {
        elem.classList.toggle('is-visible');
    }
};

const controlApi = {
    deleteRoom: async function () {
        try {
            await axios.delete(controlUrl + roomId);
        } catch (e) {
            alert(JSON.stringify(e.response.data));
        }
    },

    deleteMember: async function (roomId, memberId) {
        try {
            await axios.delete(controlUrl + roomId + "/" + memberId);
        } catch (e) {
            alert(JSON.stringify(e.response.data));
        }
    },

    createEndpoint: async function (roomId, memberId, endpointId, spec) {
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

    deleteEndpoint: async function (roomId, memberId, endpointId) {
        try {
            await axios.delete(controlUrl + roomId + '/' + memberId + '/' + endpointId);
        } catch (e) {
            alert(JSON.stringify(e.response.data));
        }
    },

    createRoom: async function (roomId) {
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

    createMember: async function (roomId, memberId, credentials) {
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

    get: async function(roomId, memberId, endpointId) {
        try {
            let url = controlUrl + roomId;
            if (memberId.length > 0 && endpointId.length > 0) {
                url = controlUrl + roomId + '/' + memberId + '/' + endpointId;
            } else if (memberId.length > 0) {
                url = controlUrl + roomId + '/' + memberId;
            }
            let resp = await axios.get(url);
            return JSON.stringify(resp.data, null, 4);
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
    'delete-room',
    'delete-member',
    'delete-endpoint',
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
