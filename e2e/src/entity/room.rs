use crate::{browser::JsExecutable, entity::Entity};

use super::Builder;
use crate::entity::CallbackSubscriber;

pub struct Room {
    id: String,
}

impl Room {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

impl Builder for Room {
    fn build(self) -> JsExecutable {
        JsExecutable::new(
            r#"
                async () => {
                    const [id] = args;

                    let jason = await window.getJason();
                    let room = await jason.init_room();
                    room.on_failed_local_media(() => {});
                    room.on_connection_loss(() => {});

                    return room;
                }
            "#,
            vec![self.id.into()],
        )
    }
}

impl Entity<Room> {
    pub async fn subscribe_on_new_connection(
        &mut self,
        sub: &mut Entity<CallbackSubscriber>,
    ) {
        self.execute_async(JsExecutable::with_objs(
            r#"
                async (room) => {
                    const [sub] = objs;

                    room.on_new_connection(() => {
                        sub.fired();
                    });
                }
            "#,
            vec![],
            vec![&sub],
        ))
        .await;
    }

    pub async fn join(&mut self, uri: String) {
        self.execute_async(JsExecutable::new(
            r#"
                async (room) => {
                    const [uri] = args;
                    await room.join(uri);
                }
            "#,
            vec![uri.into()],
        ))
        .await;
    }
}
