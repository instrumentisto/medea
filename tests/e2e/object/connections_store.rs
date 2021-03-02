//! [`Object`] storing all the [`Connection`]s thrown by
//! `Room.on_new_connection` callback.

use crate::{
    browser::Statement,
    object::{connection::Connection, Object},
};

/// Storage for [`Connection`]s thrown by `Room.on_new_connection` callback.
pub struct ConnectionStore;

impl Object<ConnectionStore> {
    /// Returns a [`Connection`] of the provided remote member.
    ///
    /// Returns [`None`] if a [`Connection`] with the provided remote member
    /// doesn't exist.
    pub async fn get(
        &self,
        remote_id: String,
    ) -> Result<Option<Object<Connection>>, super::Error> {
        let connection = self
            .execute_and_fetch(Statement::new(
                // language=JavaScript
                r#"
                    async (store) => {
                        const [id] = args;
                        return store.connections.get(id);
                    }
                "#,
                [remote_id.into()],
            ))
            .await?;

        Ok((!connection.is_undefined().await?).then(|| connection))
    }

    /// Returns a [`Connection`] for the provided remote member, waiting it if
    /// it doesn't exists at the moment.
    pub async fn wait_for_connection(
        &self,
        remote_id: String,
    ) -> Result<Object<Connection>, super::Error> {
        self.execute_and_fetch(Statement::new(
            // language=JavaScript
            r#"
                async (store) => {
                    const [remoteId] = args;
                    let conn = store.connections.get(remoteId);
                    if (conn !== undefined) {
                        return conn;
                    } else {
                        let waiter = new Promise((resolve) => {
                            store.subs.set(remoteId, resolve);
                        });
                        return await waiter;
                    }
                }
            "#,
            [remote_id.into()],
        ))
        .await
    }
}