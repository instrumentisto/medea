use crate::{browser::JsExecutable, object::Object};

pub struct LocalTrack;

impl Object<LocalTrack> {
    /// Drops this [`LocalTrack`] and returns `readyState` of the
    /// `MediaStreamTrack`.
    pub async fn free_and_check(self) -> bool {
        self.execute(JsExecutable::new(
            r#"
                async (track) => {
                    let sysTrack = track.track.get_track();
                    track.track.free();
                    return sysTrack.readyState == "ended";
                }
            "#,
            vec![],
        ))
        .await
        .unwrap()
        .as_bool()
        .unwrap()
    }

    /// Returns `MediaStreamTrack.enabled` status of underlying
    /// `MediaStreamTrack`.
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
}
