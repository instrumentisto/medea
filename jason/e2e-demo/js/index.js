let inited = false;
let wasm = null;

export async function run(credentials) {
    if (!inited) {
        wasm = await import("../../pkg");
        inited = true;
    }

    let jason = new wasm.Jason();

    jason.on_local_stream(function (stream, error) {
        if (stream) {
            let local_video = document.getElementById("yourvid");

            local_video.srcObject = stream.get_media_stream();
            local_video.play();
        } else {
            console.error(error);
        }
    });

    let room = await jason.join_room(credentials);

    let muteAudio = document.getElementsByClassName('mute-audio')[0];
    let muteVideo = document.getElementsByClassName('mute-video')[0];
    let isAudioMuted = false;
    let isVideoMuted = false;

    muteAudio.addEventListener('click', () => {
        if(isAudioMuted) {
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
        if(isVideoMuted) {
            room.unmute_video();
            isVideoMuted = false;
            muteVideo.textContent = "Mute video";
        } else {
            room.mute_video();
            isVideoMuted = true;
            muteVideo.textContent = "Unmute video";
        }
    });

    room.on_new_connection(function (connection) {
        connection.on_remote_stream(function (stream) {
            let videoDiv = document.getElementsByClassName("video-call")[0];
            var video = document.createElement("video");
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

    for(let i = 0; i < anotherMembers.length; i++) {
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

    for(let i = 0; i < memberIds.length; i++) {
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

window.onload = function () {
    try {
        var videoCallDiv = document.getElementById('video-call');
        var chooseRoomButton = document.getElementById('choose-room-btn');
        var roomIdInput = document.getElementById('room-id-input');
        let controlBtns = document.getElementsByClassName('control-btns')[0];

        var joinCallerButton = document.getElementById('join-caller');

        let usernameInput = document.getElementById('username');
        usernameInput.value = faker.name.firstName();

        var bindJoinButtons = function (roomId) {
            joinCallerButton.onclick = async function () {
                let connectBtnsDiv = document.getElementsByClassName("connect-btns")[0];
                connectBtnsDiv.style.display = 'none';
                controlBtns.style.display = 'block';

                let username = usernameInput.value;
                try {
                    let room = await axios.get(controlUrl + roomId);
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

        let roomId = window.location.hash.replace("#", "");

        bindJoinButtons(roomId);
        videoCallDiv.style.display = "";
    } catch (e) {
        console.log(e.response)
    }
};

