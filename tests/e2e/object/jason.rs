//! `Jason` JS object's representation.

use crate::{
    browser::Statement,
    object::{room::Room, Builder, Object},
};

use super::Error;

/// Representation of a `Jason` JS object.
pub struct Jason;

impl Builder for Jason {
    #[inline]
    #[must_use]
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
                        onFailedLocalStreamListener.subs =
                            onFailedLocalStreamListener.subs
                                .filter((sub) => sub());
                    });
                    let connLossListener = {
                        isLost: false,
                        reconnectHandle: null,
                        subs: []
                    };
                    room.on_connection_loss(async (recon) => {
                        connLossListener.isLost = true;
                        connLossListener.reconnectHandle = recon;
                        for (sub of connLossListener.subs) {
                            sub();
                        }
                        connLossListener.subs = [];
                    });
                    let closeListener = {
                        closeReason: null,
                        isClosed: false,
                        subs: []
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

                    let constraints = new rust.MediaStreamSettings();
                    let audio = new window.rust.AudioTrackConstraints();
                    constraints.audio(audio);
                    let video = new window.rust.DeviceVideoTrackConstraints();
                    constraints.device_video(video);
                    await room
                        .set_local_media_settings(constraints, false, false);

                    return {
                        room: room,
                        closeListener: closeListener,
                        localTracksStore: localTracksStore,
                        connLossListener: connLossListener,
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
            // language=JavaScript
            r#"
                async (jason) => {
                    const [room] = objs;
                    jason.close_room(room.room);
                }
            "#,
            [],
            [room.ptr()],
        ))
        .await
        .map(drop)
    }

    /// Drops [`Jason`] API object, so all the related objects (rooms,
    /// connections, streams, etc.) respectively.
    pub async fn dispose(self) -> Result<(), Error> {
        self.execute(Statement::new(
            // language=JavaScript
            r#"
                async (jason) => {
                    jason.dispose();
                }
            "#,
            [],
        ))
        .await
        .map(drop)
    }
}
