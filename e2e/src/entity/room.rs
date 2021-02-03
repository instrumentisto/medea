use super::Builder;
use crate::{browser::JsExecutable, entity::Entity};

pub struct Room;

impl Builder for Room {
    fn build(self) -> JsExecutable {
        JsExecutable::new(
            r#"
                async () => {
                    let jason = await window.getJason();
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
}
