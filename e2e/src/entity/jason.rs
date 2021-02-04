use crate::{
    browser::JsExecutable,
    entity::{room::Room, Builder, Entity},
};

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

impl Entity<Jason> {
    pub async fn init_room(&mut self) -> Entity<Room> {
        self.spawn_ent(JsExecutable::new(
            r#"
                async (jason) => {
                    let room = await jason.init_room();
                    room.on_failed_local_media(() => {});
                    room.on_connection_loss(() => {});
                    return room;
                }
            "#,
            vec![],
        ))
        .await
    }
}
