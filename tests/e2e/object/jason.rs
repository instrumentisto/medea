//! [`Object`] representing a `Jason` JS object.

use crate::{
    browser::Statement,
    object::{room::Room, Builder, Object},
};

use super::Error;

/// Representation of a `Jason` JS object.
pub struct Jason;

impl Builder for Jason {
    #[inline]
    fn build(self) -> Statement {
        Statement::new(
            // language=JavaScript
            r#"async () => new window.rust.Jason()"#,
            [],
        )
    }
}

impl Object<Jason> {
    /// Returns a new [`Room`] initiated in this [`Jason`] [`Object`].
    pub async fn init_room(&self) -> Result<Object<Room>, super::Error> {
        self.execute_and_fetch(Statement::new(
            // language=JavaScript
            r#"
                async (jason) => {
                    let room = await jason.init_room();
                    let onFailedLocalStreamListener = {
                        subs: [],
                        count: 0
                    };
                    room.on_failed_local_media(() => {
                        onFailedLocalStreamListener.count++;
                        onFailedLocalStreamListener.subs = onFailedLocalStreamListener.subs
                            .filter((sub) => sub());
                    });
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
                        for (sub of closeListener.subs) {
                            sub(reason);
                        }
                    });
                    room.on_local_track((t) => {
                        let track = { track: t };
                        localTracksStore.tracks.push(track);
                        let newSubs = localTracksStore.subs
                            .filter((sub) => sub(track));
                        localTracksStore.subs = newSubs;
                    });
                    room.on_connection_loss(async (recon) => {
                        while (true) {
                            try {
                                await recon.reconnect_with_delay(10);
                                break;
                            } catch(e) {}
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
                        localTracksStore: localTracksStore,
                        onFailedLocalStreamListener: onFailedLocalStreamListener
                    };
                }
            "#,
            [],
        ))
        .await
    }

    /// Closes the provided [`Room`].
    pub async fn close_room(&self, room: &Object<Room>) -> Result<(), Error> {
        self.execute(Statement::with_objs(
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
        self.execute(Statement::new(
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
