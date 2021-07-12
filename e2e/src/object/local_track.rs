//! `LocalMediaTrack` JS object's representation.

use crate::{browser::Statement, object::Object};

use super::Error;

/// Representation of a `LocalMediaTrack` object.
pub struct LocalTrack;

impl Object<LocalTrack> {
    /// Drops this [`LocalTrack`] and returns `readyState` of the underlying
    /// `MediaStreamTrack`.
    ///
    /// # Errors
    ///
    /// If failed to execute JS statement.
    pub async fn free_and_check(self) -> Result<bool, Error> {
        self.execute(Statement::new(
            // language=JavaScript
            r#"
                async (track) => {
                    let sysTrack = track.track.get_track();
                    track.track.free();
                    return sysTrack.readyState === "ended";
                }
            "#,
            [],
        ))
        .await?
        .as_bool()
        .ok_or(Error::TypeCast)
    }

    /// Returns `MediaStreamTrack.enabled` status of the underlying
    /// `MediaStreamTrack`.
    ///
    /// # Errors
    ///
    /// If failed to execute JS statement.
    pub async fn muted(&self) -> Result<bool, Error> {
        self.execute(Statement::new(
            // Not a bug, but a naming specific of WebRTC.
            // See: https:/mdn.io/Web/API/MediaStreamTrack/enabled
            // language=JavaScript
            r#"async (t) => !t.track.get_track().enabled"#,
            [],
        ))
        .await?
        .as_bool()
        .ok_or(Error::TypeCast)
    }
}
