use crate::{
    browser::JsExecutable,
    entity::{connections_store::ConnectionStore, Builder, Entity, EntityPtr},
};

pub struct Connection {
    id: String,
    connections_store: EntityPtr,
}

impl Connection {
    pub fn new(id: String, store: &Entity<ConnectionStore>) -> Self {
        Self {
            id,
            connections_store: store.ptr(),
        }
    }
}

impl Builder for Connection {
    fn build(self) -> JsExecutable {
        JsExecutable::with_objs(
            r#"
                async () => {
                    const [store] = objs;
                    const [id] = args;
                    return store.connections.get(id);
                }
            "#,
            vec![self.id.into()],
            vec![self.connections_store],
        )
    }
}
