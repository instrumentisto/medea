//! [`Object`] representing a `Room` JS object.

use std::str::FromStr;

use crate::{
    browser::Statement,
    object::{
        connections_store::ConnectionStore, tracks_store::LocalTracksStore,
        Object,
    },
};

/// Representation of a `Room` JS object.
pub struct Room;

/// Representation of a `MediaKind` JS enum.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub enum MediaKind {
    Audio,
    Video,
}

/// Error which can happen while [`MediaKind`] or [`MediaSourceKind`] parsing.
#[derive(Debug)]
pub struct FailedParsing;

impl FromStr for MediaKind {
    type Err = FailedParsing;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains("audio") {
            Ok(Self::Audio)
        } else if s.contains("video") {
            Ok(Self::Video)
        } else {
            Err(FailedParsing)
        }
    }
}

impl MediaKind {
    /// Converts this [`MediaKind`] to the JS code for this enum variant.
    pub fn as_js(self) -> String {
        match self {
            MediaKind::Audio => "window.rust.MediaKind.Audio".to_string(),
            MediaKind::Video => "window.rust.MediaKind.Video".to_string(),
        }
    }
}

/// Representation of a `MediaSourceKind` JS enum.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub enum MediaSourceKind {
    Device,
    Display,
}

impl FromStr for MediaSourceKind {
    type Err = FailedParsing;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains("device") {
            Ok(Self::Device)
        } else if s.contains("display") {
            Ok(Self::Display)
        } else {
            Err(FailedParsing)
        }
    }
}

impl MediaSourceKind {
    /// Converts this [`MediaSourceKind`] to a JS code for this enum variant.
    pub fn as_js(self) -> String {
        match self {
            MediaSourceKind::Device => "window.rust.MediaSourceKind.Device",
            MediaSourceKind::Display => "window.rust.MediaSourceKind.Display",
        }
        .to_owned()
    }
}

impl Object<Room> {
    /// Joins a [`Room`] with the provided URI.
    pub async fn join(&self, uri: String) -> Result<(), super::Error> {
        self.execute(Statement::new(
            // language=JavaScript
            r#"
                async (room) => {
                    const [uri] = args;
                    await room.room.join(uri);
                }
            "#,
            [uri.into()],
        ))
        .await?;
        Ok(())
    }

    /// Disables media publishing for the provided [`MediaKind`] and
    /// [`MediaSourceKind`].
    ///
    /// If the provided `source_kind` is [`None`], then media publishing will be
    /// disabled for all [`MediaSourceKind`]s.
    pub async fn disable_media_send(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), super::Error> {
        let media_source_kind =
            source_kind.map_or_else(String::new, MediaSourceKind::as_js);
        let disable = match kind {
            MediaKind::Audio => "room.room.disable_audio()".to_string(),
            MediaKind::Video => {
                format!("room.room.disable_video({})", media_source_kind)
            }
        };
        // language=JavaScript
        self.execute(Statement::new(
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

    /// Enables media publishing for the provided [`MediaKind`] and
    /// [`MediaSourceKind`].
    ///
    /// If provided [`None`] `source_kind` then media publishing will be
    /// enabled for all [`MediaSourceKind`]s.
    pub async fn enable_media_send(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), super::Error> {
        let media_source_kind =
            source_kind.map_or_else(String::new, MediaSourceKind::as_js);
        let disable = match kind {
            MediaKind::Audio => "room.room.enable_audio()".to_string(),
            MediaKind::Video => {
                format!("room.room.enable_video({})", media_source_kind)
            }
        };
        // language=JavaScript
        self.execute(Statement::new(
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

    /// Disables remote media receiving for the provided [`MediaKind`] and
    /// [`MediaSourceKind`].
    ///
    /// If provided [`None`] `source_kind` then media receiving will be
    /// disabled for all [`MediaSourceKind`]s.
    pub async fn disable_remote_media(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), super::Error> {
        let media_source_kind =
            source_kind.map_or_else(String::new, MediaSourceKind::as_js);
        let disable = match kind {
            MediaKind::Audio => "room.room.disable_remote_audio()".to_string(),
            MediaKind::Video => {
                format!("room.room.disable_remote_video({})", media_source_kind)
            }
        };
        // language=JavaScript
        self.execute(Statement::new(
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

    /// Enables remote media receiving for the provided [`MediaKind`] and
    /// [`MediaSourceKind`].
    ///
    /// If provided [`None`] `source_kind` then media receiving will be
    /// enabled for all [`MediaSourceKind`]s.
    pub async fn enable_remote_media(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), super::Error> {
        let media_source_kind =
            source_kind.map_or_else(String::new, MediaSourceKind::as_js);
        let disable = match kind {
            MediaKind::Audio => "room.room.enable_remote_audio()".to_string(),
            MediaKind::Video => {
                format!("room.room.enable_remote_video({})", media_source_kind)
            }
        };
        // language=JavaScript
        self.execute(Statement::new(
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

    /// Mutes media publishing for the provided [`MediaKind`] and
    /// [`MediaSourceKind`].
    ///
    /// If provided [`None`] `source_kind` then media publishing will be
    /// muted for all [`MediaSourceKind`]s.
    pub async fn mute_media(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), super::Error> {
        let media_source_kind =
            source_kind.map_or_else(String::new, MediaSourceKind::as_js);
        let disable = match kind {
            MediaKind::Audio => "room.room.mute_audio()".to_string(),
            MediaKind::Video => {
                format!("room.room.mute_video({})", media_source_kind)
            }
        };
        // language=JavaScript
        self.execute(Statement::new(
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

    /// Unmutes media publishing for the provided [`MediaKind`] and
    /// [`MediaSourceKind`].
    ///
    /// If provided [`None`] `source_kind` then media publishing will be
    /// unmuted for all [`MediaSourceKind`]s.
    pub async fn unmute_media(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), super::Error> {
        let media_source_kind =
            source_kind.map_or_else(String::new, MediaSourceKind::as_js);
        let disable = match kind {
            MediaKind::Audio => "room.room.unmute_audio()".to_string(),
            MediaKind::Video => {
                format!("room.room.unmute_video({})", media_source_kind)
            }
        };
        self.execute(Statement::new(
            // language=JavaScript
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

    /// Returns a [`ConnectionStore`] of this [`Room`].
    pub async fn connections_store(
        &self,
    ) -> Result<Object<ConnectionStore>, super::Error> {
        self.execute_and_fetch(Statement::new(
            // language=JavaScript
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
                            closeListener: closeListener,
                        };
                        conn.on_remote_track_added((t) => {
                            let track = {
                                track: t,
                                on_enabled_fire_count: 0,
                                on_disabled_fire_count: 0,
                                onEnabledSubs: [],
                                onDisabledSubs: []
                            };
                            track.track.on_enabled(() => {
                                track.on_enabled_fire_count++;
                                for (sub of track.onEnabledSubs) {
                                    sub();
                                }
                                track.onEnabledSubs = [];
                            });
                            track.track.on_disabled(() => {
                                track.on_disabled_fire_count++;
                                for (sub of track.onDisabledSubs) {
                                    sub();
                                }
                                track.onDisabledSubs = [];
                            });
                            tracksStore.tracks.push(track);
                            let newStoreSubs = tracksStore.subs
                                .filter((sub) => {
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
                        if (sub !== undefined) {
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

    /// Returns this [`Room`]'s [`LocalTrack`]s store.
    ///
    /// [`LocalTrack`]: crate::object::local_track::LocalTrack
    pub async fn local_tracks(
        &self,
    ) -> Result<Object<LocalTracksStore>, super::Error> {
        // language=JavaScript
        Ok(self
            .execute_and_fetch(Statement::new(
                r#"
                    async (room) => {
                        return room.localTracksStore;
                    }
                "#,
                vec![],
            ))
            .await?)
    }

    /// Returns [`Future`] which will be resolved when `Room.on_close` callback
    /// will fire.
    ///
    /// [`Future`]: std::future::Future
    pub async fn wait_for_close(&self) -> Result<String, super::Error> {
        // language=JavaScript
        Ok(self
            .execute(Statement::new(
                r#"
                    async (room) => {
                        if (room.closeListener.isClosed) {
                            return room.closeListener.closeReason.reason();
                        } else {
                            let waiter = new Promise((resolve) => {
                                room.closeListener.subs.push(resolve);
                            });

                            let closeReason = await waiter;
                            return closeReason.reason();
                        }
                    }
                "#,
                vec![],
            ))
            .await?
            .as_str()
            .ok_or(super::Error::TypeCast)?
            .to_string())
    }
}
