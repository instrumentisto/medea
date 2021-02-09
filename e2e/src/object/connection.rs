use crate::{
    browser::JsExecutable,
    object::{track_store::TrackStore, Object},
};
use crate::object::room::{MediaKind, MediaSourceKind};
use crate::object::track::Track;
use futures::Stream;

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
        )).await.unwrap()
    }
}
