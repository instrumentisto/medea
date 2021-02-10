use crate::{browser::JsExecutable, object::Object};

pub struct Track;

impl Object<Track> {
    pub async fn enabled(&self) -> bool {
        self.execute(JsExecutable::new(
            r#"
                async (track) => {
                    return track.enabled();
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
                    console.log(track.get_track());
                    return track.get_track().enabled;
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
