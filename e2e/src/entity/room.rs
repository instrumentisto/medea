use crate::{
    browser::JsExecutable,
    entity::{connections_store::ConnectionStore, Entity},
};

pub struct Room;

pub enum MediaKind {
    Audio,
    Video,
}

pub enum MediaSourceKind {
    Device,
    Display,
}

impl Entity<Room> {
    pub async fn join(&mut self, uri: String) {
        self.execute(JsExecutable::new(
            r#"
                async (room) => {
                    const [uri] = args;
                    await room.join(uri);
                }
            "#,
            vec![uri.into()],
        ))
        .await
        .unwrap();
    }

    pub async fn disable_media(
        &mut self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) {
        let media_source_kind =
            source_kind.map_or("", |source_kind| match source_kind {
                MediaSourceKind::Device => "window.rust.MediaSourceKind.DEVICE",
                MediaSourceKind::Display => {
                    "window.rust.MediaSourceKind.DISPLAY"
                }
            });
        let disable = match kind {
            MediaKind::Audio => "room.disable_audio()".to_string(),
            MediaKind::Video => {
                format!("room.disable_video({})", media_source_kind)
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
        .await
        .unwrap();
    }

    pub async fn connections_store(&mut self) -> Entity<ConnectionStore> {
        self.spawn_entity(JsExecutable::new(
            r#"
                async (room) => {
                    let store = {
                        connections: new Map(),
                        subs: new Map(),
                    };
                    room.on_new_connection((conn) => {
                        let id = conn.get_remote_member_id();
                        store.connections.set(id, conn);
                        let sub = store.subs.get(id);
                        if (sub != undefined) {
                            sub(conn);
                        }
                    });

                    return store;
                }
            "#,
            vec![],
        ))
        .await
    }
}
