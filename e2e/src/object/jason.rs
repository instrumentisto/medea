//! Implementation and definition for the object which represents `Jason` JS
//! object.

use crate::{
    browser::Statement,
    object::{room::Room, Builder, Object},
};

/// Representation of the `Jason` JS object.
pub struct Jason;

impl Builder for Jason {
    fn build(self) -> Statement {
        Statement::new(
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

impl Object<Jason> {
    /// Returns new [`Room`] initiated in this [`Jason`].
    pub async fn init_room(&self) -> Result<Object<Room>, super::Error> {
        self.execute_and_fetch(Statement::new(
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
