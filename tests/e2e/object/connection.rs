/// Representation of the `Connection` JS object.
use crate::{
    browser::Statement,
    object::{tracks_store::RemoteTracksStore, Object},
};

use super::Error;

/// Representation of the `Connection` JS object.
pub struct Connection;

impl Object<Connection> {
    /// Returns [`RemoteTracksStore`] of this [`Connection`].
    pub async fn tracks_store(
        &self,
    ) -> Result<Object<RemoteTracksStore>, Error> {
        // language=JavaScript
        Ok(self
            .execute_and_fetch(Statement::new(
                r#"
                    async (conn) => {
                        return conn.tracksStore;
                    }
                "#,
                vec![],
            ))
            .await?)
    }

    /// Returns [`Future`] which will be resolved when `Connection.on_close`
    /// callback will fire.
    ///
    /// [`Future`]: std::future::Future
    pub async fn wait_for_close(&self) -> Result<(), Error> {
        // language=JavaScript
        self.execute(Statement::new(
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
            vec![],
        ))
        .await?;
        Ok(())
    }
}
