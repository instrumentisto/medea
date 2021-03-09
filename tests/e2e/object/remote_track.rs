//! `RemoteMediaTrack` JS object's representation.

use crate::{browser::Statement, object::Object};

use super::Error;

/// Representation of a `RemoteMediaTrack` object.
pub struct RemoteTrack;

impl Object<RemoteTrack> {
    /// Waits for this [`RemoteTrack`] being enabled.
    pub async fn wait_for_enabled(&self) -> Result<(), Error> {
        self.execute(Statement::new(
            // language=JavaScript
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
            [],
        ))
        .await
        .map(|_| ())
    }

    /// Waits for this [`RemoteTrack`] being disabled, or the
    /// `RemoteMediaTrack.on_disabled()` callback to fire.
    pub async fn wait_for_disabled(&self) -> Result<(), Error> {
        self.execute(Statement::new(
            // language=JavaScript
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
            [],
        ))
        .await
        .map(|_| ())
    }

    /// Indicates whether this [`RemoteTrack`]'s underlying `MediaStreamTrack`
    /// is disabled.
    pub async fn disabled(&self) -> Result<bool, Error> {
        self.execute(Statement::new(
            // language=JavaScript
            r#"async (t) => !t.track.get_track().enabled"#,
            [],
        ))
        .await?
        .as_bool()
        .ok_or(Error::TypeCast)
    }

    /// Waits for the `RemoteMediaTrack.on_disabled()` callback to fire `count`
    /// times.
    pub async fn wait_for_on_disabled_fire_count(
        &self,
        count: u64,
    ) -> Result<(), Error> {
        self.execute(Statement::new(
            // language=JavaScript
            r#"
                async (track) => {
                    const [count] = args;
                    while (track.on_disabled_fire_count !== count) {
                        await new Promise((resolve) => {
                            if (track.on_disabled_fire_count !== count) {
                                track.onDisabledSubs.push(resolve);
                            } else {
                                resolve();
                            }
                        });
                    }
                }
            "#,
            [count.into()],
        ))
        .await
        .map(|_| ())
    }

    /// Waits for the `RemoteMediaTrack.on_enabled()` callback to fire `count`
    /// times.
    pub async fn wait_for_on_enabled_fire_count(
        &self,
        count: u64,
    ) -> Result<(), Error> {
        self.execute(Statement::new(
            // language=JavaScript
            r#"
                async (track) => {
                    const [count] = args;
                    while (track.on_enabled_fire_count !== count) {
                        await new Promise((resolve) => {
                            if (track.on_enabled_fire_count !== count) {
                                track.onEnabledSubs.push(resolve);
                            } else {
                                resolve();
                            }
                        });
                    }
                }
            "#,
            [count.into()],
        ))
        .await
        .map(|_| ())
    }
}
