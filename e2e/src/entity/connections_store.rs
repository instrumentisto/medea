use crate::entity::{EntityPtr, Builder};
use crate::browser::JsExecutable;

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
                    };
                    room.on_new_connection((conn) => {
                        store.connections.set(conn.get_remote_member_id(), conn);
                    });

                    return store;
                }
            "#,
            vec![],
            vec![self.room],
        )
    }
}