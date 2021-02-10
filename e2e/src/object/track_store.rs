use crate::{
    browser::JsExecutable,
    object::{
        room::{MediaKind, MediaSourceKind},
        track::Track,
        Object,
    },
};

pub struct TrackStore;

impl Object<TrackStore> {
    pub async fn has_track(
        &self,
        kind: MediaKind,
        source_kind: MediaSourceKind,
    ) -> bool {
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

        self.execute(kind_js.and_then(JsExecutable::new(
            r#"
            async (meta) => {
                for (track of meta.store.tracks) {
                    if (track.track.kind() === meta.kind && track
                        .track.media_source_kind() === meta
                        .sourceKind) {
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
    ) -> Object<Track> {
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
                        let isKindEq = track.track.kind() === meta.kind;
                        let isSourceKindEq = track.track.media_source_kind()
                            === meta.sourceKind;
                        if (isKindEq && isSourceKindEq) {
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
