//! Implementation and definition for the object which represents `Jason` JS
//! object.

use crate::{
    browser::JsExecutable,
    object::{room::Room, Builder, Object},
};

use super::Error;

/// Representation of the `Jason` JS object.
pub struct Jason;

impl Builder for Jason {
    fn build(self) -> JsExecutable {
        JsExecutable::new(
            r#"
                async () => {
                    let jason = new window.rust.Jason();
                    return jason;
                }
            "#,
            vec![],
        )
    }
}

impl Object<Jason> {
    /// Returns new [`Room`] initiated in this [`Jason`].
    pub async fn init_room(&self) -> Result<Object<Room>, Error> {
        self.spawn_object(JsExecutable::new(
            r#"
                async (jason) => {
                    let room = await jason.init_room();
                    room.on_failed_local_media(() => {});
                    room.on_connection_loss(() => {});
                    let closeListener = {
                        closeReason: null,
                        isClosed: false,
                        subs: [],
                    };
                    let localTracksStore = {
                        tracks: [],
                        subs: []
                    };
                    room.on_close((reason) => {
                        closeListener.closeReason = reason;
                        closeListener.isClosed = true;
                        for (sub of subs) {
                            sub(reason);
                        }
                    });
                    room.on_local_track((t) => {
                        let track = { track: t };
                        localTracksStore.tracks.push(track);
                        for (sub of room.localTracksStore.subs) {
                            sub(track);
                        }
                    });

                    let constraints = new rust.MediaStreamSettings();
                    let audio = new window.rust.AudioTrackConstraints();
                    constraints.audio(audio);
                    let video = new window.rust.DeviceVideoTrackConstraints();
                    constraints.device_video(video);
                    room.set_local_media_settings(constraints, false, false);

                    return {
                        room: room,
                        closeListener: closeListener,
                        localTracksStore: localTracksStore
                    };
                }
            "#,
            vec![],
        ))
        .await
    }

    /// Closes the provided [`Room`].
    pub async fn close_room(&self, room: &Object<Room>) -> Result<(), Error> {
        self.execute(JsExecutable::with_objs(
            r#"
                async (jason) => {
                    const [room] = objs;
                    jason.close_room(room.room);
                }
            "#,
            vec![],
            vec![room.ptr()],
        ))
        .await?;
        Ok(())
    }

    /// Drops [`Jason`] API object, so all related objects (rooms, connections,
    /// streams etc.) respectively.
    pub async fn dispose(self) -> Result<(), Error> {
        self.execute(JsExecutable::new(
            r#"
                async (jason) => {
                    jason.dispose();
                }
            "#,
            vec![],
        ))
        .await?;
        Ok(())
    }
}
