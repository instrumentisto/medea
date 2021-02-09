//! Implementation and definition for the object which represents `Room` JS
//! object.

use crate::{
    browser::JsExecutable,
    object::{connections_store::ConnectionStore, Object},
};

/// Representation of the `Room` JS object.
pub struct Room;

/// Representation of the `MediaKind` JS enum.
pub enum MediaKind {
    Audio,
    Video,
}

impl MediaKind {
    pub fn as_js(&self) -> String {
        match self {
            MediaKind::Audio => {
                "window.rust.MediaKind.Audio".to_string()
            }
            MediaKind::Video => {
                "window.rust.MediaKind.Video".to_string()
            }
        }
    }
}

/// Representation of the `MediaSourceKind` JS enum.
#[allow(dead_code)]
pub enum MediaSourceKind {
    Device,
    Display,
}

impl MediaSourceKind {
    /// Converts this [`MediaSourceKind`] to the JS code for this enum variant.
    pub fn as_js(&self) -> String {
        match self {
            MediaSourceKind::Device => {
                "window.rust.MediaSourceKind.Device".to_string()
            }
            MediaSourceKind::Display => {
                "window.rust.MediaSourceKind.Display".to_string()
            }
        }
    }
}

impl Object<Room> {
    /// Joins [`Room`] with a provided URI.
    pub async fn join(&self, uri: String) -> Result<(), super::Error> {
        self.execute(JsExecutable::new(
            r#"
                async (room) => {
                    const [uri] = args;
                    await room.room.join(uri);
                }
            "#,
            vec![uri.into()],
        ))
        .await?;

        Ok(())
    }

    /// Disabled media publishing for the provided [`MediaKind`] and
    /// [`MediaSourceKind`].
    ///
    /// If provided [`None`] `source_kind` then media publishing will be
    /// disabled for all [`MediaSourceKind`]s.
    pub async fn disable_media(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), super::Error> {
        let media_source_kind = source_kind
            .as_ref()
            .map_or_else(String::new, MediaSourceKind::as_js);
        let disable = match kind {
            MediaKind::Audio => "room.room.disable_audio()".to_string(),
            MediaKind::Video => {
                format!("room.room.disable_video({})", media_source_kind)
            }
        };
        self.execute(JsExecutable::new(
            &format!(
                r#"
                async (room) => {{
                    await {};
                }}
            "#,
                disable
            ),
            vec![],
        ))
        .await?;

        Ok(())
    }

    pub async fn enable_media(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), super::Error> {
        let media_source_kind = source_kind
            .as_ref()
            .map_or_else(String::new, MediaSourceKind::as_js);
        let disable = match kind {
            MediaKind::Audio => "room.room.enable_audio()".to_string(),
            MediaKind::Video => {
                format!("room.room.enable_video({})", media_source_kind)
            }
        };
        self.execute(JsExecutable::new(
            &format!(
                r#"
                async (room) => {{
                    await {};
                }}
            "#,
                disable
            ),
            vec![],
        ))
            .await?;

        Ok(())
    }

    pub async fn mute_media(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), super::Error> {
        let media_source_kind = source_kind
            .as_ref()
            .map_or_else(String::new, MediaSourceKind::as_js);
        let disable = match kind {
            MediaKind::Audio => "room.room.mute_audio()".to_string(),
            MediaKind::Video => {
                format!("room.room.mute_video({})", media_source_kind)
            }
        };
        self.execute(JsExecutable::new(
            &format!(
                r#"
                async (room) => {{
                    await {};
                }}
            "#,
                disable
            ),
            vec![],
        ))
            .await?;

        Ok(())
    }

    pub async fn unmute_media(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), super::Error> {
        let media_source_kind = source_kind
            .as_ref()
            .map_or_else(String::new, MediaSourceKind::as_js);
        let disable = match kind {
            MediaKind::Audio => "room.room.unmute_audio()".to_string(),
            MediaKind::Video => {
                format!("room.room.unmute_video({})", media_source_kind)
            }
        };
        self.execute(JsExecutable::new(
            &format!(
                r#"
                async (room) => {{
                    await {};
                }}
            "#,
                disable
            ),
            vec![],
        ))
            .await?;

        Ok(())
    }

    /// Returns [`ConnectionStore`] for this [`Room`].
    pub async fn connections_store(
        &self,
    ) -> Result<Object<ConnectionStore>, super::Error> {
        self.spawn_object(JsExecutable::new(
            r#"
                async (room) => {
                    let store = {
                        connections: new Map(),
                        subs: new Map(),
                    };
                    room.room.on_new_connection((conn) => {
                        let closeListener = {
                            isClosed: false,
                            subs: [],
                        };
                        let tracksStore = {
                            tracks: [],
                            subs: []
                        };
                        let connection = {
                            conn: conn,
                            tracksStore: tracksStore,
                            closeListener: closeListener
                        };
                        conn.on_remote_track_added((track) => {
                            tracksStore.tracks.push(track);
                            let newStoreSubs = tracksStore.subs.filter((sub) => {
                                return sub(track);
                            });
                            tracksStore.subs = newStoreSubs;
                        });
                        conn.on_close(() => {
                            closeListener.isClosed = true;
                            for (sub of closeListener.subs) {
                                sub();
                            }
                        });
                        let id = conn.get_remote_member_id();
                        store.connections.set(id, connection);
                        let sub = store.subs.get(id);
                        if (sub != undefined) {
                            sub(connection);
                        }
                    });

                    return store;
                }
            "#,
            vec![],
        ))
        .await
    }

    pub async fn wait_for_close(&self) -> String {
        self.execute(JsExecutable::new(
            r#"
                async (room) => {
                    if (room.closeListener.isClosed) {
                        return room.closeListener.closeReason.reason();
                    } else {
                        let waiter = new Promise((resolve, reject) => {
                            room.closeListener.subs.push(resolve);
                        });

                        let closeReason = await waiter;
                        return closeReason.reason();
                    }
                }
            "#,
            vec![]
        )).await.unwrap().as_str().unwrap().to_string()
    }
}
