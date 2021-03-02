//! [`Object`] representing a `Room` JS object.

use crate::{
    browser::Statement,
    object::{connections_store::ConnectionStore, Object},
};

/// Representation of a `Room` JS object.
pub struct Room;

/// Representation of a `MediaKind` JS enum.
pub enum MediaKind {
    Audio,
    Video,
}

/// Representation of a `MediaSourceKind` JS enum.
pub enum MediaSourceKind {
    Device,
    Display,
}

impl MediaSourceKind {
    /// Converts this [`MediaSourceKind`] to a JS code for this enum variant.
    fn as_js(&self) -> String {
        match self {
            MediaSourceKind::Device => "window.rust.MediaSourceKind.DEVICE",
            MediaSourceKind::Display => "window.rust.MediaSourceKind.DISPLAY",
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
                    await room.join(uri);
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
        let media_source_kind = source_kind
            .as_ref()
            .map_or_else(String::new, MediaSourceKind::as_js);
        let disable = match kind {
            MediaKind::Audio => "room.disable_audio()".to_owned(),
            MediaKind::Video => {
                format!("room.disable_video({})", media_source_kind)
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
                disable,
            ),
            [],
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
                        subs: new Map()
                    };
                    room.on_new_connection((conn) => {
                        let id = conn.get_remote_member_id();
                        store.connections.set(id, conn);
                        let sub = store.subs.get(id);
                        if (sub !== undefined) {
                            sub(conn);
                        }
                    });
                    return store;
                }
            "#,
            [],
        ))
        .await
    }
}
