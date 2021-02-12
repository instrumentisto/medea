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

pub struct TracksStore<T>(PhantomData<T>);

impl<T> Object<TracksStore<T>> {
    /// Returns count of [`LocalTrack`]s stored in this [`LocalTracksStore`].
    pub async fn count(&self) -> Result<u64, Error> {
        Ok(self
            .execute(Statement::new(
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

    /// Returns [`Future`] which will be resolved when count of `MediaTrack`s
    /// will be same as provided one.
    pub async fn wait_for_count(&self, count: u64) -> Result<(), Error> {
        self.execute(Statement::new(
            r#"
                async (store) => {
                    const [neededCount] = args;
                    let currentCount = store.tracks.length;
                    if (currentCount >= neededCount) {
                        return;
                    } else {
                        let waiter = new Promise((resolve, reject) => {
                            store.subs.push((track) => {
                                currentCount += 1;
                                if (currentCount >= neededCount) {
                                    resolve();
                                    return true;
                                }
                                return false;
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

    /// Returns `true` if this [`LocalTracksStore`] contains [`LocalTrack`] with
    /// a provided [`MediaKind`] and [`MediaSourceKind`].
    pub async fn has_track(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<bool, Error> {
        let source_kind_js = source_kind
            .map_or_else(|| "undefined".to_string(), MediaSourceKind::as_js);
        let kind_js = Statement::new(
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

    /// Returns [`LocalTrack`] from this [`LocalTracksStore`] with a provided
    /// [`MediaKind`] and [`MediaSourceKind`].
    pub async fn get_track(
        &self,
        kind: MediaKind,
        source_kind: MediaSourceKind,
    ) -> Result<Object<T>, Error> {
        let kind_js = Statement::new(
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
