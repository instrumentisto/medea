//! [`Object`] representing a `Jason` JS object.

use crate::{
    browser::Statement,
    object::{room::Room, Builder, Object},
};

/// Representation of a `Jason` JS object.
pub struct Jason;

impl Builder for Jason {
    #[inline]
    fn build(self) -> Statement {
        Statement::new(
            // language=JavaScript
            r#"async () => new window.rust.Jason()"#,
            [],
        )
    }
}

impl Object<Jason> {
    /// Returns a new [`Room`] initiated in this [`Jason`] [`Object`].
    pub async fn init_room(&self) -> Result<Object<Room>, super::Error> {
        self.execute_and_fetch(Statement::new(
            // language=JavaScript
            r#"
                async (jason) => {
                    let room = await jason.init_room();
                    room.on_failed_local_media(() => {});
                    room.on_connection_loss(() => {});
                    return room;
                }
            "#,
            [],
        ))
        .await
    }
}
