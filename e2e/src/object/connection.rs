//! `Connection` JS object's representation.

use crate::{
    browser::Statement,
    object::{tracks_store::RemoteTracksStore, Object},
};

use super::Error;

/// Representation of a `Connection` JS object.
pub struct Connection;

impl Object<Connection> {
    /// Returns a [`RemoteTracksStore`] of this [`Connection`].
    ///
    /// # Errors
    ///
    /// If failed to execute JS statement.
    pub async fn tracks_store(
        &self,
    ) -> Result<Object<RemoteTracksStore>, Error> {
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
