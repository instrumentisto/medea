//! Implementation and definition for the object which represents `Jason` JS
//! object.

use crate::{
    browser::JsExecutable,
    object::{room::Room, Builder, Object},
};

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
    pub async fn init_room(&self) -> Result<Object<Room>, super::Error> {
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
                    room.on_local_track((track) => {
                        localTracksStore.tracks.push(track);
                        for (sub of room.localTracksStore.subs) {
                            sub(track);
                        }
                    });

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

    pub async fn close_room(&self, room: &Object<Room>) {
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
        .await
        .unwrap();
    }

    pub async fn dispose(self) {
        self.execute(JsExecutable::new(
            r#"
                async (jason) => {
                    jason.dispose();
                }
            "#,
            vec![],
        ))
        .await
        .unwrap();
    }
}
