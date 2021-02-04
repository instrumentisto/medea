use super::Builder;
use crate::{browser::JsExecutable, entity::Entity};
use crate::entity::jason::Jason;

pub struct Room;

impl Builder for Room {
    fn build(self) -> JsExecutable {
        JsExecutable::new(
            r#"
                async () => {
                    const [jason] = objs;
                    let room = await jason.init_room();
                    room.on_failed_local_media(() => {});
                    room.on_connection_loss(() => {});

                    return room;
                }
            "#,
            vec![],
        )
    }
}

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
        let media_source_kind = if let Some(source_kind) = source_kind {
            match source_kind {
                MediaSourceKind::Device => "window.rust.MediaSourceKind.DEVICE",
                MediaSourceKind::Display => {
                    "window.rust.MediaSourceKind.DISPLAY"
                }
            }
        } else {
            ""
        };
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
        )).await.unwrap();
    }
}
