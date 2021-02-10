use crate::{browser::JsExecutable, object::Object};

pub struct Track;

impl Object<Track> {
    pub async fn enabled(&self) -> bool {
        self.execute(JsExecutable::new(
            r#"
                async (track) => {
                    return track.track.enabled();
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
                    return track.track.get_track().enabled;
                }
            "#,
            vec![],
        ))
        .await
        .unwrap()
        .as_bool()
        .unwrap()
    }

    pub async fn on_enabled_fire_count(&self) -> u64 {
        self.execute(JsExecutable::new(
            r#"
                async (track) => {
                    return track.on_enabled_fire_count;
                }
            "#,
            vec![]
        )).await.unwrap().as_u64().unwrap()
    }

    pub async fn on_disabled_fire_count(&self) -> u64 {
        self.execute(JsExecutable::new(
            r#"
                async (track) => {
                    return track.on_disabled_fire_count;
                }
            "#,
            vec![]
        )).await.unwrap().as_u64().unwrap()
    }
}
