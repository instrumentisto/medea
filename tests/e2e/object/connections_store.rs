//! Implementation and definition for the object which will store all
//! [`Connection`]s thrown by `Room.on_new_connection` callback.

use crate::{
    browser::Statement,
    object::{connection::Connection, Object},
};

/// Storage for the [`Connection`]s thrown by `Room.on_new_connection` callback.
pub struct ConnectionStore;

impl Object<ConnectionStore> {
    /// Returns [`Connection`] for the provided remote Member ID.
    ///
    /// Returns [`None`] if [`Connection`] with a provided remote Member ID is
    /// not exist.
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
                vec![remote_id.into()],
            ))
            .await?;

        Ok(if connection.is_undefined().await? {
            None
        } else {
            Some(connection)
        })
    }

    /// Returns [`Connection`] for the provided remote Member ID.
    ///
    /// If this [`Connection`] currently not exists then this method will wait
    /// for it.
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
                    if (conn != undefined) {
                        return conn;
                    } else {
                        let waiter = new Promise((resolve, reject) => {
                            store.subs.set(remoteId, resolve);
                        });
                        return await waiter;
                    }
                }
            "#,
            vec![remote_id.into()],
        ))
        .await
    }
}
