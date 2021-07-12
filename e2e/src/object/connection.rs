//! `Connection` JS object's representation.

use crate::{
    browser::Statement,
    object::{tracks_store, Object},
};

use super::Error;

/// Representation of a `Connection` JS object.
pub struct Connection;

impl Object<Connection> {
    /// Returns a [`tracks_store::Remote`] of this [`Connection`].
    ///
    /// # Errors
    ///
    /// If failed to execute JS statement.
    pub async fn tracks_store(
        &self,
    ) -> Result<Object<tracks_store::Remote>, Error> {
        self.execute_and_fetch(Statement::new(
            // language=JavaScript
            r#"async (conn) => conn.tracksStore"#,
            [],
        ))
        .await
    }

    /// Returns a [`Future`] resolving when `Connection.on_close()` callback is
    /// fired.
    ///
    /// # Errors
    ///
    /// If failed to execute JS statement.
    ///
    /// [`Future`]: std::future::Future
    pub async fn wait_for_close(&self) -> Result<(), Error> {
        self.execute(Statement::new(
            // language=JavaScript
            r#"
                async (conn) => {
                    await new Promise((resolve) => {
                        if (!conn.closeListener.isClosed) {
                            conn.closeListener.subs.push(resolve);
                        } else {
                            resolve();
                        }
                    });
                }
            "#,
            [],
        ))
        .await
        .map(drop)
    }
}
