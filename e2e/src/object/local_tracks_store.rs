use crate::{
    browser::JsExecutable,
    object::{
        local_track::LocalTrack,
        room::{MediaKind, MediaSourceKind},
        Object,
    },
};

pub struct LocalTracksStore;

impl Object<LocalTracksStore> {
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

    pub async fn has_track(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> bool {
        let source_kind_js =
            source_kind.map_or_else(|| "undefined".to_string(), |k| k.as_js());
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
                    if (track.kind() === meta.kind
                        && (track.media_source_kind() === meta.sourceKind
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

    pub async fn get_track(
        &self,
        kind: MediaKind,
        source_kind: MediaSourceKind,
    ) -> Object<LocalTrack> {
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
                        if (track.kind() === meta.kind
                            && track.media_source_kind() === meta.sourceKind) {
                            return track;
                        }
                    }
                    let waiter = new Promise((resolve, reject) => {
                        meta.store.subs.push((track) => {
                            let kind = track.kind();
                            let sourceKind = track.media_source_kind();
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
