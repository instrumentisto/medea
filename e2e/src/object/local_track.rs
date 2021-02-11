use crate::{browser::JsExecutable, object::Object};

pub struct LocalTrack;

impl Object<LocalTrack> {
    pub async fn free_and_check(self) -> bool {
        self.execute(JsExecutable::new(
            r#"
                async (track) => {
                    let sysTrack = track.get_track();
                    track.free();
                    return sysTrack.readyState == "ended";
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
