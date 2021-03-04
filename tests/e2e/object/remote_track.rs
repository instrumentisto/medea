//! Representation of the `RemoteMediaTrack` JS object.

use crate::{browser::Statement, object::Object};

use super::Error;

/// Representation of the `RemoteMediaTrack` object.
pub struct RemoteTrack;

impl Object<RemoteTrack> {
    /// Returns `true` if this [`RemoteTrack`] is enabled.
    pub async fn wait_for_enabled(&self) -> Result<(), Error> {
        // language=JavaScript
        self.execute(Statement::new(
            r#"
                async (track) => {
                    if (!track.track.enabled()) {
                        let waiter = new Promise((resolve) => {
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
        // language=JavaScript
        self.execute(Statement::new(
            r#"
                async (track) => {
                    if (track.track.enabled()) {
                        let waiter = new Promise((resolve) => {
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
        // language=JavaScript
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

    /// Returns [`Future`] which will be resolved when count of
    /// `RemoteMediaTrack.on_disabled` fires will be same as provided one.
    ///
    /// [`Future`]: std::future::Future
    pub async fn wait_for_on_disabled_fire_count(
        &self,
        count: u64,
    ) -> Result<(), Error> {
        // language=JavaScript
        self.execute(Statement::new(
            r#"
                async (track) => {
                    const [count] = args;
                    while (track.on_disabled_fire_count != count) {
                        await new Promise((resolve) => {
                            if (track.on_disabled_fire_count != count) {
                                track.onDisabledSubs.push(resolve);
                            } else {
                                resolve();
                            }
                        });
                    }
                }
            "#,
            vec![count.into()],
        ))
        .await?;
        Ok(())
    }

    /// Returns [`Future`] which will be resolved when count of
    /// `RemoteMediaTrack.on_enabled` fires will be same as provided one.
    ///
    /// [`Future`]: std::future::Future
    pub async fn wait_for_on_enabled_fire_count(
        &self,
        count: u64,
    ) -> Result<(), Error> {
        // language=JavaScript
        self.execute(Statement::new(
            r#"
                async (track) => {
                    const [count] = args;
                    while (track.on_enabled_fire_count != count) {
                        await new Promise((resolve) => {
                            if (track.on_enabled_fire_count != count) {
                                track.onEnabledSubs.push(resolve);
                            } else {
                                resolve();
                            }
                        });
                    }
                }
            "#,
            vec![count.into()],
        ))
        .await?;
        Ok(())
    }
}
