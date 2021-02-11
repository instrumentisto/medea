use std::marker::PhantomData;

use crate::{
    browser::JsExecutable,
    object::{
        local_track::LocalTrack,
        remote_track::RemoteTrack,
        room::{MediaKind, MediaSourceKind},
        Object,
    },
};

pub type LocalTracksStore = TracksStore<LocalTrack>;
pub type RemoteTracksStore = TracksStore<RemoteTrack>;

pub struct TracksStore<T>(PhantomData<T>);

impl<T> Object<TracksStore<T>> {
    /// Returns count of [`LocalTrack`]s stored in this [`LocalTracksStore`].
    pub async fn count(&self) -> u64 {
        self.execute(JsExecutable::new(
            r#"
                async (store) => {
                    return store.tracks.length;
                }
            "#,
            vec![],
        ))
        .await
        .unwrap()
        .as_u64()
        .unwrap()
    }

    /// Returns `true` if this [`LocalTracksStore`] contains [`LocalTrack`] with
    /// a provided [`MediaKind`] and [`MediaSourceKind`].
    pub async fn has_track(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> bool {
        let source_kind_js = source_kind
            .map_or_else(|| "undefined".to_string(), MediaSourceKind::as_js);
        let kind_js = JsExecutable::new(
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

        self.execute(kind_js.and_then(JsExecutable::new(
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
        .await
        .unwrap()
        .as_bool()
        .unwrap()
    }

    /// Returns [`LocalTrack`] from this [`LocalTracksStore`] with a provided
    /// [`MediaKind`] and [`MediaSourceKind`].
    pub async fn get_track(
        &self,
        kind: MediaKind,
        source_kind: MediaSourceKind,
    ) -> Object<T> {
        let kind_js = JsExecutable::new(
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

        self.spawn_object(kind_js.and_then(JsExecutable::new(
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
        .await
        .unwrap()
    }
}
