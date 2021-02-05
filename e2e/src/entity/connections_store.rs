use crate::{
    browser::JsExecutable,
    entity::{connection::Connection, Builder, Entity, EntityPtr},
};

pub struct ConnectionStore {
    room: EntityPtr,
}

impl Builder for ConnectionStore {
    fn build(self) -> JsExecutable {
        JsExecutable::with_objs(
            r#"
                async () => {
                    const [room] = objs;
                    let store = {
                        connections: new Map(),
                        subs: new Map(),
                    };
                    room.on_new_connection((conn) => {
                        let id = conn.get_remote_member_id();
                        store.connections.set(id, conn);
                        let sub = store.subs.get(id);
                        if (sub != undefined) {
                            sub(conn);
                        }
                    });

                    return store;
                }
            "#,
            vec![],
            vec![self.room],
        )
    }
}

impl Entity<ConnectionStore> {
    pub async fn get(
        &mut self,
        remote_id: String,
    ) -> Option<Entity<Connection>> {
        let mut connection = self
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
        &mut self,
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
