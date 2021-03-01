use crate::browser::{Statement, Window};

pub struct Gum<'a>(pub(super) &'a Window);

impl<'a> Gum<'a> {
    pub(super) async fn instantiate(window: &Window) {
        window
            .execute(Statement::new(
                r#"
                async () => {
                    window.gumMock = {
                        original: navigator.mediaDevices.getUserMedia
                    };
                }
            "#,
                vec![],
            ))
            .await
            .unwrap();
    }

    pub async fn broke_gum(&self, video: bool, audio: bool) {
        self.0
            .execute(Statement::new(
                r#"
                async () => {
                    const [isVideoBroken, isAudioBroken] = args;
                    navigator.mediaDevices.getUserMedia = async (cons) => {
                        if (isAudioBroken && cons.audio != null) {
                            throw new NotFoundError();
                        }
                        if (isVideoBroken && cons.video != null) {
                            throw new NotFoundError();
                        }
                        return await window.gumMock.original(cons);
                    }
                }
            "#,
                vec![video.into(), audio.into()],
            ))
            .await
            .unwrap();
    }

    pub async fn unbroke_gum(&self) {
        self.0.execute(Statement::new(
            r#"
                async () => {
                    navigator.mediaDevices.getUserMedia = window.gumMock.original;
                }
            "#,
            vec![]
        )).await.unwrap();
    }
}
