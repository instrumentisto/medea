use crate::{browser::JsExecutable, object::Object};

use super::Error;

pub struct LocalTrack;

impl Object<LocalTrack> {
    /// Drops this [`LocalTrack`] and returns `readyState` of the
    /// `MediaStreamTrack`.
    pub async fn free_and_check(self) -> Result<bool, Error> {
        Ok(self
            .execute(JsExecutable::new(
                r#"
                async (track) => {
                    let sysTrack = track.track.get_track();
                    track.track.free();
                    return sysTrack.readyState == "ended";
                }
            "#,
                vec![],
            ))
            .await?
            .as_bool()
            .ok_or(Error::TypeCast)?)
    }

    /// Returns `MediaStreamTrack.enabled` status of underlying
    /// `MediaStreamTrack`.
    pub async fn muted(&self) -> Result<bool, Error> {
        Ok(self
            .execute(JsExecutable::new(
                r#"
                async (track) => {
                    return !track.track.get_track().enabled;
                }
            "#,
                vec![],
            ))
            .await?
            .as_bool()
            .ok_or(Error::TypeCast)?)
    }
}
