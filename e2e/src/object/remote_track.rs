use crate::{browser::JsExecutable, object::Object};

use super::Error;

pub struct RemoteTrack;

impl Object<RemoteTrack> {
    /// Returns `true` if this [`Track`] is enabled.
    pub async fn enabled(&self) -> Result<bool, Error> {
        Ok(self
            .execute(JsExecutable::new(
                r#"
                async (track) => {
                    return track.track.enabled();
                }
            "#,
                vec![],
            ))
            .await?
            .as_bool()
            .ok_or(Error::TypeCast)?)
    }

    /// Returns `true` if this [`Track`] underlying `MediaStreamTrack.enabled`
    /// if `false`.
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

    /// Returns count of `RemoteMediaTrack.on_enabled` callback fires.
    pub async fn on_enabled_fire_count(&self) -> Result<u64, Error> {
        Ok(self
            .execute(JsExecutable::new(
                r#"
                async (track) => {
                    return track.on_enabled_fire_count;
                }
            "#,
                vec![],
            ))
            .await?
            .as_u64()
            .ok_or(Error::TypeCast)?)
    }

    /// Returns count of `RemoteMediaTrack.on_disabled` callback fires.
    pub async fn on_disabled_fire_count(&self) -> Result<u64, Error> {
        Ok(self
            .execute(JsExecutable::new(
                r#"
                async (track) => {
                    return track.on_disabled_fire_count;
                }
            "#,
                vec![],
            ))
            .await?
            .as_u64()
            .ok_or(Error::TypeCast)?)
    }
}
