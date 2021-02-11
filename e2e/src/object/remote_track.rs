use crate::{browser::JsExecutable, object::Object};

pub struct RemoteTrack;

impl Object<RemoteTrack> {
    /// Returns `true` if this [`Track`] is enabled.
    pub async fn enabled(&self) -> bool {
        self.execute(JsExecutable::new(
            r#"
                async (track) => {
                    return track.track.enabled();
                }
            "#,
            vec![],
        ))
        .await
        .unwrap()
        .as_bool()
        .unwrap()
    }

    /// Returns `true` if this [`Track`] underlying `MediaStreamTrack.enabled`
    /// if `false`.
    pub async fn muted(&self) -> bool {
        self.execute(JsExecutable::new(
            r#"
                async (track) => {
                    return !track.track.get_track().enabled;
                }
            "#,
            vec![],
        ))
        .await
        .unwrap()
        .as_bool()
        .unwrap()
    }

    /// Returns count of `RemoteMediaTrack.on_enabled` callback fires.
    pub async fn on_enabled_fire_count(&self) -> u64 {
        self.execute(JsExecutable::new(
            r#"
                async (track) => {
                    return track.on_enabled_fire_count;
                }
            "#,
            vec![],
        ))
        .await
        .unwrap()
        .as_u64()
        .unwrap()
    }

    /// Returns count of `RemoteMediaTrack.on_disabled` callback fires.
    pub async fn on_disabled_fire_count(&self) -> u64 {
        self.execute(JsExecutable::new(
            r#"
                async (track) => {
                    return track.on_disabled_fire_count;
                }
            "#,
            vec![],
        ))
        .await
        .unwrap()
        .as_u64()
        .unwrap()
    }
}
