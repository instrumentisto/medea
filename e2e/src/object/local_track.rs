use crate::{browser::JsExecutable, object::Object};

pub struct LocalTrack;

impl Object<LocalTrack> {
    pub async fn dispose_and_check(self) -> bool {
        self.execute(JsExecutable::new(
            r#"
                async (track) => {
                    let sysTrack = track.get_track();
                    track.dispose();
                    return sysTrack.muted;
                }
            "#,
            vec![],
        ))
        .await
        .unwrap()
        .as_bool()
        .unwrap()
    }

    pub async fn muted(&self) -> bool {
        self.execute(JsExecutable::new(
            r#"
                async (track) => {
                    return !track.get_track().enabled;
                }
            "#,
            vec![],
        ))
        .await
        .unwrap()
        .as_bool()
        .unwrap()
    }
}
