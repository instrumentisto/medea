//! Implementation and definition of store for the [`LocalTrack`]s and
//! [`RemoteTrack`]s.

use std::marker::PhantomData;

use crate::{
    browser::Statement,
    object::{
        local_track::LocalTrack,
        remote_track::RemoteTrack,
        room::{MediaKind, MediaSourceKind},
        Object,
    },
};

use super::Error;

pub type LocalTracksStore = TracksStore<LocalTrack>;
pub type RemoteTracksStore = TracksStore<RemoteTrack>;

/// Store for the [`LocalTrack`]s or [`RemoteTrack`]s.
pub struct TracksStore<T>(PhantomData<T>);

impl<T> Object<TracksStore<T>> {
    /// Returns count of Tracks stored in this [`TracksStore`].
    pub async fn count(&self) -> Result<u64, Error> {
        Ok(self
            .execute(Statement::new(
                // language=JavaScript
                r#"
                async (store) => {
                    return store.tracks.length;
                }
            "#,
                vec![],
            ))
            .await?
            .as_u64()
            .ok_or(Error::TypeCast)?)
    }

    /// Returns [`Future`] which will be resolved when count of Tracks
    /// will be same as provided one.
    ///
    /// [`Future`]: std::future::Future
    pub async fn wait_for_count(&self, count: u64) -> Result<(), Error> {
        self.execute(Statement::new(
            // language=JavaScript
            r#"
                async (store) => {
                    const [neededCount] = args;
                    let currentCount = store.tracks.length;
                    if (currentCount === neededCount) {
                        return;
                    } else {
                        let waiter = new Promise((resolve, reject) => {
                            store.subs.push((track) => {
                                currentCount += 1;
                                if (currentCount === neededCount) {
                                    resolve();
                                    return false;
                                }
                                return true;
                            });
                        });
                        await waiter;
                    }
                }
            "#,
            vec![count.into()],
        ))
        .await?;
        Ok(())
    }

    /// Returns `true` if this [`TracksStore`] contains Track with
    /// a provided [`MediaKind`] and [`MediaSourceKind`].
    pub async fn has_track(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<bool, Error> {
        let source_kind_js = source_kind
            .map_or_else(|| "undefined".to_string(), MediaSourceKind::as_js);
        let kind_js = Statement::new(
            // language=JavaScript
            &format!(
                r#"
                async (store) => {{
                    return {{
                        store: store,
                        kind: {kind},
                        sourceKind: {source_kind}
                    }};
                }}
            "#,
                source_kind = source_kind_js,
                kind = kind.as_js()
            ),
            vec![],
        );

        Ok(self
            .execute(kind_js.and_then(Statement::new(
                // language=JavaScript
                r#"
            async (meta) => {
                for (track of meta.store.tracks) {
                    if (track.track.kind() === meta.kind
                        && (track.track.media_source_kind() === meta.sourceKind
                            || meta.sourceKind === undefined)) {
                        return true;
                    }
                }
                return false;
            }
        "#,
                vec![],
            )))
            .await?
            .as_bool()
            .ok_or(Error::TypeCast)?)
    }

    /// Returns Track from this [`TracksStore`] with a provided [`MediaKind`]
    /// and [`MediaSourceKind`].
    pub async fn get_track(
        &self,
        kind: MediaKind,
        source_kind: MediaSourceKind,
    ) -> Result<Object<T>, Error> {
        let kind_js = Statement::new(
            // language=JavaScript
            &format!(
                r#"
                async (store) => {{
                    return {{
                        store: store,
                        kind: {kind},
                        sourceKind: {source_kind}
                    }};
                }}
            "#,
                source_kind = source_kind.as_js(),
                kind = kind.as_js()
            ),
            vec![],
        );

        Ok(self
            .execute_and_fetch(kind_js.and_then(Statement::new(
                // language=JavaScript
                r#"
                async (meta) => {
                    for (track of meta.store.tracks) {
                        let kind = track.track.kind();
                        let sourceKind = track.track.media_source_kind();
                        if (kind === meta.kind
                            && sourceKind === meta.sourceKind) {
                            return track;
                        }
                    }
                    let waiter = new Promise((resolve, reject) => {
                        meta.store.subs.push((track) => {
                            let kind = track.track.kind();
                            let sourceKind = track.track.media_source_kind();
                            if (kind === meta.kind
                                && sourceKind === meta.sourceKind) {
                                resolve(track);
                                return true;
                            } else {
                                return false;
                            }
                        });
                    });
                    let res = await waiter;
                    return waiter;
                }
            "#,
                vec![],
            )))
            .await?)
    }
}
