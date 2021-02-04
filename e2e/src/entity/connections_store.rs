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
        let mut connection = self.spawn_ent(JsExecutable::new(
            r#"
                async (store) => {
                    const [id] = args;
                    return store.connections.get(id);
                }
            "#,
            vec![remote_id.into()],
        )).await;

        if connection.is_undefined().await {
            None
        } else {
            Some(connection)
        }
    }
}
