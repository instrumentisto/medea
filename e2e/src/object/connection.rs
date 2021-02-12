use crate::{
    browser::JsExecutable,
    object::{tracks_store::RemoteTracksStore, Object},
};

/// Representation of the `Connection` JS object.
pub struct Connection;

impl Object<Connection> {
    /// Returns [`TrackStore`] of this [`Connection`].
    pub async fn tracks_store(&self) -> Object<RemoteTracksStore> {
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

    /// Returns [`Future`] which will be resolved when `Connection.on_close`
    /// callback will fire.
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
