//! Implementation and definition for the object which represents `Jason` JS
//! object.

use crate::{
    browser::JsExecutable,
    entity::{room::Room, Builder, Entity},
};

/// Representation of the `Jason` JS object.
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
    /// Returns new [`Room`] initiated in this [`Jason`].
    pub async fn init_room(&self) -> Result<Entity<Room>, super::Error> {
        self.spawn_entity(JsExecutable::new(
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
