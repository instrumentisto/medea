use crate::{
    browser::JsExecutable,
    object::{track_store::TrackStore, Object},
};

/// Representation of the `Connection` JS object.
pub struct Connection;

impl Object<Connection> {
    pub async fn tracks_store(&self) -> Object<TrackStore> {
        self.spawn_object(JsExecutable::new(
            r#"
                async (conn) => {
                    return conn.tracksStore;
                }
            "#,
            vec![],
        ))
        .await
        .unwrap()
    }

    pub async fn wait_for_close(&self) {
        self.execute(JsExecutable::new(
            r#"
                async (conn) => {
                    if (!conn.closeListener.isClosed) {
                        await new Promise((resolve, reject) => {
                            conn.closeListener.subs.push(resolve);
                        });
                    }
                }
            "#,
            vec![],
        ))
        .await
        .unwrap();
    }
}
