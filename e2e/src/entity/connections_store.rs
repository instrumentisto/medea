use crate::{
    browser::JsExecutable,
    entity::{connection::Connection, Entity},
};

pub struct ConnectionStore;

impl Entity<ConnectionStore> {
    pub async fn get(&self, remote_id: String) -> Option<Entity<Connection>> {
        let connection = self
            .spawn_entity(JsExecutable::new(
                r#"
                async (store) => {
                    const [id] = args;
                    return store.connections.get(id);
                }
            "#,
                vec![remote_id.into()],
            ))
            .await;

        if connection.is_undefined().await {
            None
        } else {
            Some(connection)
        }
    }

    pub async fn wait_for_connection(
        &self,
        remote_id: String,
    ) -> Entity<Connection> {
        self.spawn_entity(JsExecutable::new(
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
