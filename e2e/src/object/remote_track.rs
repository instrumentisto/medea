use crate::{browser::Statement, object::Object};

use super::Error;

/// Representation of the `RemoteMediaTrack` object.
pub struct RemoteTrack;

impl Object<RemoteTrack> {
    /// Returns `true` if this [`RemoteTrack`] is enabled.
    pub async fn wait_for_enabled(&self) -> Result<(), Error> {
        self.execute(Statement::new(
            r#"
                async (track) => {
                    if (!track.track.enabled()) {
                        let waiter = new Promise((resolve, reject) => {
                            track.onEnabledSubs.push(resolve);
                        });
                        await waiter;
                    }
                }
            "#,
            vec![],
        ))
        .await?;
        Ok(())
    }

    /// Returns [`Future`] which will be resolved id `RemoteMediaTrack.enabled`
    /// will be `false` or when `RemoteMediaTrack.on_disabled` callback will
    /// fire.
    ///
    /// [`Future`]: std::future::Future
    pub async fn wait_for_disabled(&self) -> Result<(), Error> {
        self.execute(Statement::new(
            r#"
                async (track) => {
                    if (track.track.enabled()) {
                        let waiter = new Promise((resolve, reject) => {
                            track.onDisabledSubs.push(resolve);
                        });
                        await waiter;
                    }
                }
            "#,
            vec![],
        ))
        .await?;
        Ok(())
    }

    /// Returns `true` if this [`RemoteTrack`] underlying
    /// `MediaStreamTrack.enabled` is `false`.
    pub async fn muted(&self) -> Result<bool, Error> {
        Ok(self
            .execute(Statement::new(
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
            .execute(Statement::new(
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
            .execute(Statement::new(
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
